//! Criterion throughput benchmarks — feeds the GitHub Pages dashboard.
//!
//! Unlike `benches/real_time.rs` (which is a pass/fail CI gate on
//! allocations), this bench produces Criterion's HTML reports under
//! `target/criterion/`, published by the `pages` CI job on every push to
//! `master`. It measures wall-clock throughput of the same allocation-free
//! reference processor at both standard block sizes, so regressions are
//! visible as a trend rather than just a gate.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use tpt_av_test_benchmark::audio_block::{BLOCK_SIZE_1024, BLOCK_SIZE_512, DEFAULT_CHANNELS};

fn reference_processor(buffer: &mut [f32], frame_index: u64) {
    let fade = ((frame_index % 64) as f32 + 1.0) / 64.0;
    for sample in buffer.iter_mut() {
        let shaped = *sample * fade;
        *sample = shaped.clamp(-0.999, 0.999);
    }
}

fn bench_audio_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_block_throughput");
    for block_size in [BLOCK_SIZE_512, BLOCK_SIZE_1024] {
        let samples = block_size * DEFAULT_CHANNELS;
        group.throughput(criterion::Throughput::Elements(samples as u64));
        group.bench_with_input(
            BenchmarkId::new("reference_processor", block_size),
            &block_size,
            |b, _| {
                let mut buffer = vec![0.5_f32; samples];
                let mut frame_index = 0u64;
                b.iter(|| {
                    reference_processor(black_box(&mut buffer), frame_index);
                    frame_index = frame_index.wrapping_add(1);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_audio_block);
criterion_main!(benches);
