# Contributing

Please open an issue before starting work on a pull request, including bug
fixes and small changes. This gives maintainers a chance to weigh in on
approach before you invest the time, and avoids duplicate or conflicting
work.

## Before opening a PR

Run these locally; all of them are CI gates and a PR won't merge until they
pass:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo deny check --all-features
```

If your change touches `tpt-av-test-fuzz/fuzz/`, also run the fuzz target
for a short burst locally:

```bash
cargo fuzz run --fuzz-dir tpt-av-test-fuzz/fuzz pcm_bytes_to_f32 -- -max_total_time=60
```

## Licensing

Every dependency must resolve to MIT, Apache-2.0, BSD-2-Clause,
BSD-3-Clause, ISC, Zlib, or CC0-1.0 (see `deny.toml`). GPL/LGPL/AGPL/MPL
dependencies are never acceptable, including transitively. If your change
needs to compare against a copyleft reference tool (e.g. FFmpeg), invoke it
as a CLI subprocess — never link it.

Contributions are accepted under the same dual MIT OR Apache-2.0 license as
the rest of the project (see [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE)).
