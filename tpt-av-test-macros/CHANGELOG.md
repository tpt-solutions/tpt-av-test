# Changelog

All notable changes to `tpt-av-test-macros` are documented in this file.

## [0.1.0] — 2026-09-21

Initial release.

- `#[bench_real_time]` attribute macro: runs the annotated function under
  `tpt-av-test-benchmark`'s allocation tracker, with an optional
  `max_duration` wall-clock budget and a `name` label for failure messages.
