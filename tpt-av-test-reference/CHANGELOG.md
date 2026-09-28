# Changelog

All notable changes to `tpt-av-test-reference` are documented in this file.

## [0.1.0] — 2026-09-21

Initial release.

- FFmpeg CLI subprocess wrapper (`ffmpeg`): spawn, stream raw PCM over stdout.
- Bit-exact PCM comparison with float epsilon (`audio::assert_bit_exact_vs_ffmpeg`).
- Pixel-exact frame comparison with first-diff reporting (`image::assert_frame_exact`).
- PSNR/SSIM image comparison (`image::assert_image_similar`).
- libsndfile/sox/ffprobe CLI wrapper (`libsndfile`).
- Official WAV, FLAC, Opus, and ITU-T H.264 conformance vectors, plus
  reference compositor images, tracked via Git LFS in `tpt-av-test-vectors`.
