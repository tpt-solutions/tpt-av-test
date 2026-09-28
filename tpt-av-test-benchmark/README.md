# tpt-av-test-benchmark

Real-time safety and performance benchmarking for the [TPT AV Stack](https://opensource.tptsolutions.co.nz/).

A custom global allocator that counts heap allocations, microsecond-precision
execution timing, simulated audio callbacks at the standard 512/1024-frame
block sizes, and the `assert_real_time_safe!` macro plus `#[bench_real_time]`
attribute that fail a test the moment a real-time path allocates.

This crate is intended for use via `[dev-dependencies]`, `cfg(test)`, and
dedicated `benches/` targets only.

## Modules

- `allocation_tracker` — custom global allocator tracking heap allocations.
- `timing` — microsecond-precision execution timing.
- `audio_block` — simulated audio callback benchmarking (512/1024 samples).
- `macros` — `#[bench_real_time]` proc macro + `assert_real_time_safe!`.

## Usage

```toml
[dev-dependencies]
tpt-av-test-benchmark = { git = "https://github.com/tpt-solutions/tpt-av-test" }
```

```rust
use tpt_av_test_benchmark::assert_real_time_safe;

assert_real_time_safe!({
    let mut buffer = [0.0f32; 512];
    for sample in &mut buffer {
        *sample = *sample * 0.5 + 0.25;
    }
});
```

### Global allocator

By default this crate installs `TrackingAllocator` as the process
`#[global_allocator]` (`tracking-allocator` feature), so simply depending on
it makes every allocation observable. Binaries that need their own allocator
build the crate with `default-features = false` and declare
`TrackingAllocator` themselves — there can only be one `#[global_allocator]`
per binary.

## License

Dual-licensed under [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE), at your option.

See the [workspace README](../README.md) for the full project vision and layout.
