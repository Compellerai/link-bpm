# link-bpm

Unofficial clean-room Rust crate for reading the BPM advertised by an Ableton Link session on the local network.

Compeller built this because REACT needed a lightweight way to follow the tempo of a Link session for live visuals, without pulling in the full Ableton Link SDK. We are busy building Compeller, but this utility should be useful to other Rust, music, lighting, and show-control projects, so we are sharing it.

## Current scope

- Passively listen for Ableton Link UDP multicast packets on `224.76.78.75:20808`.
- Parse the session tempo from the timeline payload and expose it in BPM.
- Run a background listener thread and let your app poll the latest reading.
- Parse a single raw Link packet directly with `parse_link_tempo`.

## Not included

- No Ableton Link SDK, source code, or third-party Link implementation.
- No beat phase, quantum, start/stop, or session-state sync.
- No peer presence or session joining — this crate only listens, it never announces itself.
- No internet or SaaS calls.

## Quick start

Add the crate:

```toml
[dependencies]
link-bpm = "0.1"
```

Print the current Link BPM:

```bash
cargo run --example print_bpm
```

Minimal use:

```rust
use std::{thread, time::Duration};
use link_bpm::LinkBpmListener;

fn main() -> std::io::Result<()> {
    let listener = LinkBpmListener::new()?;

    loop {
        match listener.tempo() {
            Some(reading) => println!("{:.2} BPM", reading.bpm),
            None => println!("waiting for a Link session..."),
        }
        thread::sleep(Duration::from_secs(1));
    }
}
```

Or parse a raw packet yourself:

```rust
use link_bpm::parse_link_tempo;

if let Some(bpm) = parse_link_tempo(packet) {
    println!("{bpm:.2} BPM");
}
```

## Compatibility

link-bpm reads the tempo that any Link-enabled app broadcasts on the LAN (Ableton Live and other Link-capable software and hardware). Reports of what it does and does not read cleanly are welcome.

## Safety note

The listener binds UDP port `20808` and joins the Link multicast group `224.76.78.75` on your local interfaces. It is passive: it only reads multicast traffic already on your LAN and never announces itself as a Link peer or sends session data, so it will not disturb other Link peers. It does not call Compeller services and does not use the internet.

## Clean-room note

This crate was built from observed network traffic and contains no Ableton source code, no Ableton Link SDK, and no third-party Link implementation. The packet tests ship synthetic packets, not captured Ableton data.

It reads tempo only and is not a full or compliant Link implementation. It is not affiliated with, endorsed by, or sponsored by Ableton.

"Ableton" and "Link" are trademarks of their respective owners and are used here only to describe protocol compatibility.

## Contributing

We are especially interested in:

- packet fixtures and compatibility reports from more Link-enabled apps and hardware
- a JSON or event-stream example
- an async API

Issues and pull requests are welcome.

## License

Licensed under either of:

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
