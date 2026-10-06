#![no_main]

use libfuzzer_sys::fuzz_target;
use midici_pe::{
    parse_get_inquiry_header, parse_notify_header, parse_set_inquiry_header,
    parse_subscription_header,
};

fuzz_target!(|data: &[u8]| {
    // PE JSON header parsing must never panic: the size (4096 B), 7-bit, and
    // nesting-depth guards reject before serde runs, and malformed JSON maps
    // to PeStatus::BadRequest. // M2-103 §7.4.1 / ARD §7
    let _ = parse_get_inquiry_header(data);
    let _ = parse_set_inquiry_header(data);
    let _ = parse_subscription_header(data);
    let _ = parse_notify_header(data);
});
