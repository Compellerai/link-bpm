//! Unofficial clean-room passive BPM listener for Ableton Link sessions.
//!
//! This crate does one small thing: it listens for Link UDP multicast packets
//! on the local network and exposes the session tempo in BPM.
//!
//! It is not a full Link implementation. It does not join session state, sync
//! beat phase, schedule quantum changes, or include any Ableton source code.
//! The packet parser was built from observed network traffic.

use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Ableton Link multicast group. The last three octets are ASCII L, N, K.
pub const LINK_GROUP: Ipv4Addr = Ipv4Addr::new(224, 76, 78, 75);

/// Ableton Link UDP port.
pub const LINK_PORT: u16 = 20808;

const LINK_MAGIC: &[u8] = b"_asdp_v";
const TMLN_KEY: &[u8] = b"tmln";

/// One tempo reading from a Link session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoReading {
    /// Session tempo in beats per minute.
    pub bpm: f32,
    /// Local time when the packet carrying this tempo was received.
    pub received_at: Instant,
}

#[derive(Clone, Copy, Debug, Default)]
struct LinkState {
    tempo: Option<TempoReading>,
}

/// Options for [`LinkBpmListener`].
#[derive(Clone, Debug)]
pub struct ListenerOptions {
    /// UDP receive timeout. The listener uses this to notice shutdown.
    pub read_timeout: Duration,
    /// Join multicast on all local IPv4 interfaces when supported.
    pub join_all_interfaces: bool,
}

impl Default for ListenerOptions {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_millis(500),
            join_all_interfaces: true,
        }
    }
}

/// Passive background listener for Link BPM packets.
///
/// Construct one listener and poll [`LinkBpmListener::tempo`] from your app.
/// Dropping the listener requests the background thread to stop.
pub struct LinkBpmListener {
    state: Arc<Mutex<LinkState>>,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LinkBpmListener {
    /// Start listening with default options.
    pub fn new() -> std::io::Result<Self> {
        Self::with_options(ListenerOptions::default())
    }

    /// Start listening with custom options.
    pub fn with_options(options: ListenerOptions) -> std::io::Result<Self> {
        let socket = open_socket(&options)?;
        let state = Arc::new(Mutex::new(LinkState::default()));
        let running = Arc::new(AtomicBool::new(true));
        let thread_state = Arc::clone(&state);
        let thread_running = Arc::clone(&running);
        let worker = thread::Builder::new()
            .name("link-bpm-listener".to_string())
            .spawn(move || listener_loop(socket, thread_state, thread_running))?;

        Ok(Self {
            state,
            running,
            worker: Some(worker),
        })
    }

    /// Latest BPM reading, if a Link tempo packet has been observed.
    pub fn tempo(&self) -> Option<TempoReading> {
        self.state.lock().ok()?.tempo
    }
}

impl Drop for LinkBpmListener {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Parse BPM from a raw Link UDP payload.
///
/// The `tmln` timeline entry carries a big-endian microseconds-per-beat value.
/// BPM is `60_000_000 / micros_per_beat`.
pub fn parse_link_tempo(payload: &[u8]) -> Option<f32> {
    if !payload.starts_with(LINK_MAGIC) {
        return None;
    }
    let key_at = payload
        .windows(TMLN_KEY.len())
        .position(|w| w == TMLN_KEY)?;
    let value_at = key_at + TMLN_KEY.len() + 4;
    let tempo_bytes: [u8; 8] = payload.get(value_at..value_at + 8)?.try_into().ok()?;
    let micros_per_beat = u64::from_be_bytes(tempo_bytes);
    if micros_per_beat == 0 {
        return None;
    }
    Some((60_000_000.0 / micros_per_beat as f64) as f32)
}

fn open_socket(options: &ListenerOptions) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, LINK_PORT)).into())?;
    let socket: UdpSocket = socket.into();

    let mut joined = 0usize;
    if options.join_all_interfaces {
        for iface in multicast_interfaces() {
            match socket.join_multicast_v4(&LINK_GROUP, &iface) {
                Ok(()) => {
                    joined += 1;
                    log::debug!("joined {LINK_GROUP}:{LINK_PORT} on {iface}");
                }
                Err(error) => log::debug!("could not join {LINK_GROUP} on {iface}: {error}"),
            }
        }
    }

    if joined == 0 {
        socket.join_multicast_v4(&LINK_GROUP, &Ipv4Addr::UNSPECIFIED)?;
    }

    socket.set_read_timeout(Some(options.read_timeout))?;
    Ok(socket)
}

#[cfg(unix)]
fn multicast_interfaces() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return out;
        }
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() && i32::from((*ifa.ifa_addr).sa_family) == libc::AF_INET {
                let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                if !ip.is_unspecified() && !ip.is_link_local() {
                    out.push(ip);
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(ifap);
    }
    out
}

#[cfg(not(unix))]
fn multicast_interfaces() -> Vec<Ipv4Addr> {
    Vec::new()
}

fn listener_loop(socket: UdpSocket, state: Arc<Mutex<LinkState>>, running: Arc<AtomicBool>) {
    let mut buf = [0u8; 2048];
    while running.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buf) {
            Ok((len, _src)) => {
                if let Some(bpm) = parse_link_tempo(&buf[..len]) {
                    if let Ok(mut state) = state.lock() {
                        state.tempo = Some(TempoReading {
                            bpm,
                            received_at: Instant::now(),
                        });
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => {
                log::warn!("Link BPM listener stopped after socket error: {error}");
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet_with_tempo(micros_per_beat: u64) -> Vec<u8> {
        let mut packet = Vec::new();
        packet.extend_from_slice(LINK_MAGIC);
        packet.extend_from_slice(&[0x01, 0x01, 0x05, 0x00]);
        packet.extend_from_slice(b"sess");
        packet.extend_from_slice(&8u32.to_be_bytes());
        packet.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        packet.extend_from_slice(TMLN_KEY);
        packet.extend_from_slice(&24u32.to_be_bytes());
        packet.extend_from_slice(&micros_per_beat.to_be_bytes());
        packet.extend_from_slice(&[0; 16]);
        packet
    }

    #[test]
    fn decodes_120_bpm() {
        let bpm = parse_link_tempo(&packet_with_tempo(500_000)).unwrap();
        assert!((bpm - 120.0).abs() < 0.001, "got {bpm}");
    }

    #[test]
    fn decodes_fractional_bpm() {
        let bpm = parse_link_tempo(&packet_with_tempo(512_821)).unwrap();
        assert!((bpm - 117.0).abs() < 0.01, "got {bpm}");
    }

    #[test]
    fn rejects_non_link_payload() {
        assert!(parse_link_tempo(b"not a link packet").is_none());
    }

    #[test]
    fn rejects_truncated_payload() {
        let packet = packet_with_tempo(500_000);
        assert!(parse_link_tempo(&packet[..20]).is_none());
    }

    #[test]
    fn rejects_zero_tempo() {
        assert!(parse_link_tempo(&packet_with_tempo(0)).is_none());
    }
}
