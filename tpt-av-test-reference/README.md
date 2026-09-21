# tpt-av-test-reference

Golden-master comparison harness for the [TPT AV Stack](https://opensource.tptsolutions.co.nz/).

Proves that pure-Rust decoders and compositors match industry-standard
references (FFmpeg, libsndfile, sox, ffprobe) **without linking** their
GPL/LGPL code — every reference tool is invoked as a detached CLI subprocess
and its output streamed over piped stdout, so their licenses never enter this
MIT/Apache-2.0 codebase.

This crate is `[dev-dependencies]` / `cfg(test)` only. It must never appear
in a production binary's dependency graph.

## Modules

- `ffmpeg` — FFmpeg CLI subprocess wrapper (decodes to raw PCM on stdout).
- `libsndfile` — libsndfile/sox/ffprobe CLI wrapper.
- `audio` — PCM bit-exact comparison with float epsilon (`assert_bit_exact_vs_ffmpeg`).
- `image` — pixel-exact frame comparison (`assert_frame_exact`) and PSNR/SSIM comparison (`assert_image_similar`).

## Usage

```toml
[dev-dependencies]
tpt-av-test-reference = { git = "https://github.com/tpt-solutions/tpt-av-test" }
```

```rust,ignore
use tpt_av_test_reference::audio::assert_bit_exact_vs_ffmpeg;

#[test]
fn decoder_matches_ffmpeg() {
    assert_bit_exact_vs_ffmpeg("tests/fixtures/tone.wav", |path| {
        my_decoder::decode_to_pcm(path)
    });
}
```

Requires `ffmpeg` (and, for `libsndfile` module functions, `sndfile-info`/`sox`/`ffprobe`)
to be present on `PATH`.

## License

Dual-licensed under [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE), at your option.

See the [workspace README](../README.md) for the full project vision and layout.
