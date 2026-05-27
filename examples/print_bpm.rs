use link_bpm::LinkBpmListener;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> std::io::Result<()> {
    env_logger::init();
    let listener = LinkBpmListener::new()?;
    loop {
        match listener.tempo() {
            Some(reading) => {
                let age = Instant::now()
                    .duration_since(reading.received_at)
                    .as_secs_f32();
                println!("{:.2} BPM, heard {:.1}s ago", reading.bpm, age);
            }
            None => println!("waiting for Link BPM..."),
        }
        thread::sleep(Duration::from_secs(1));
    }
}
