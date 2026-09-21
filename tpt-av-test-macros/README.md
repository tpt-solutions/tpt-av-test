# tpt-av-test-macros

Procedural macros backing the [`tpt-av-test-benchmark`](../tpt-av-test-benchmark)
harness for the [TPT AV Stack](https://opensource.tptsolutions.co.nz/).

Currently provides `bench_real_time`, an attribute macro that turns a test or
benchmark function into a real-time safety gate: the body runs under
`tpt-av-test-benchmark`'s allocation tracker and, optionally, a wall-clock
budget, and the function fails the moment a heap allocation (or budget
overrun) is detected.

This crate is not meant to be depended on directly — it is re-exported as
`tpt_av_test_benchmark::bench_real_time`.

## Usage

```rust,ignore
#[test]
#[tpt_av_test_macros::bench_real_time(max_duration = "11ms")]
fn mixer_512_sample_callback_is_rt_safe() {
    let mut buffer = vec![0.0f32; 512 * 2];
    for sample in &mut buffer {
        *sample = *sample * 0.5 + 0.25; // pretend mixing
    }
}
```

### Options

- `max_duration = "<duration>"` — additionally assert that the body finishes
  within the given duration (e.g. `"10ms"`, `"512us"`).
- `name = "<label>"` — label used in failure messages (defaults to the
  function name).

## License

Dual-licensed under [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE), at your option.

See the [workspace README](../README.md) for the full project vision and layout.
