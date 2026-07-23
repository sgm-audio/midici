//! Standalone virtual ALSA UMP MIDI-CI responder daemon.
//!
//! Full daemon wiring lands in later phases (`docs/ARD-001.md` §9 Slice 0+).

fn main() {
    println!("{} {}", env!("CARGO_PKG_NAME"), midici_responder::VERSION);
}
