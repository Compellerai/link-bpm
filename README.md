# link-bpm

Unofficial clean-room Rust crate for reading the current BPM advertised by an Ableton Link session on the LAN.

This is intentionally small:

- passive UDP multicast listener
- current BPM only
- parser for raw Link tempo packets
- no Ableton SDK
- no Ableton source code
- no third-party Link implementation
- no beat phase, quantum, start/stop, or session sync

It is for apps that just need to answer: "what BPM is the local Link session advertising?"

## Install

```toml
[dependencies]
link-bpm = "0.1"
```

## Use

```rust
use link_bpm::LinkBpmListener;

let listener = LinkBpmListener::new()?;

if let Some(reading) = listener.tempo() {
    println!("{:.2} BPM", reading.bpm);
}
# Ok::<(), std::io::Error>(())
```

## Example

```bash
cargo run --example print_bpm
```

## Clean-room note

This crate was built from observed network packets and contains no Ableton source code. It is not affiliated with, endorsed by, or sponsored by Ableton.

"Ableton" and "Link" are trademarks of their respective owners. They are used here only to describe protocol compatibility.

## License

Licensed under either of:

- MIT license
- Apache License, Version 2.0

at your option.
