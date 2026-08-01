//! CLAP auto-property demo plugin host-side binary scaffold.
//!
//! Plugin RT bridge and `ChCtrlList` mapping land in later phases
//! (`docs/ARD-001.md` §5 / §9 Slice 3).

fn main() {
    println!("{} {}", env!("CARGO_PKG_NAME"), midici_responder::VERSION);
}
