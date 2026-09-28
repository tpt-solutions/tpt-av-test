#![no_main]

use libfuzzer_sys::fuzz_target;
use tpt_av_test_reference::ffmpeg::pcm_bytes_to_f32;

// Proves `pcm_bytes_to_f32` never panics on arbitrary bytes read off an
// ffmpeg subprocess's stdout pipe — the one routine in this workspace that
// parses raw external bytes outside of test/mock code.
fuzz_target!(|data: &[u8]| {
    let _ = pcm_bytes_to_f32(data.to_vec());
});
