#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = semasm_decode::decode_x86_64(data, 0x1000);
    let _ = semasm_decode::decode_aarch64(data, 0x1000);
});
