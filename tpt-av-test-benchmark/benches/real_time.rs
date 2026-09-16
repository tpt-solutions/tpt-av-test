//! Real-time safety benchmarks — the CI `cargo bench` gate.
//!
//! Runs the standard 512- and 1024-frame simulated audio callbacks with an
//! allocation-free reference processor. Any heap allocation inside a
//! callback fails the process (exit code != 0) and therefore the CI `bench`
//! job. A generous wall-clock sanity ceiling (1000x the block duration, so
//! shared CI hosts cannot trip it spuriously) guards against pathological
//! slowdowns; the *hard* gate is zero allocations.
//!
//! The gate also self-checks: it verifies that the harness *would* flag a
//! deliberately allocating processor, so a silent regression in the tracker
//! itself cannot let real violations through.

use tpt_av_test_benchmark::audio_block::{
    benchmark_audio_block, AudioBlockContext, BLOCK_SIZE_1024, BLOCK_SIZE_512,
};

/// Allocation-free stereo reference processor: a slow deterministic fade with
/// a soft clip, pure arithmetic on the driver-provided buffer.
fn reference_processor(buffer: &mut [f32], context: AudioBlockContext) {
    let fade = ((context.frame_index % 64) as f32 + 1.0) / 64.0;
    for sample in buffer.iter_mut() {
        let shaped = *sample * fade;
        *sample = shaped.clamp(-0.999, 0.999);
    }
}

fn main() {
    let mut failures = 0usize;

    for block_size in [BLOCK_SIZE_512, BLOCK_SIZE_1024] {
        let iterations = if cfg!(debug_assertions) { 32 } else { 512 };
        let result = benchmark_audio_block(block_size, iterations, reference_processor);
        println!("bench/{block_size}: {result}");

        // Hard gate: not a single heap allocation across every callback.
        if std::panic::catch_unwind(|| result.assert_real_time_safe()).is_err() {
            eprintln!("bench/{block_size}: FAIL — heap allocations inside the callback");
            failures += 1;
        }

        // Generous wall-clock sanity ceiling (CI hosts are shared and slow).
        let ceiling = result.block_duration() * 1000;
        if result.stats.max() > ceiling {
            eprintln!(
                "bench/{block_size}: FAIL — slowest callback {:?} exceeded sanity ceiling {:?}",
                result.stats.max(),
                ceiling
            );
            failures += 1;
        }
    }

    // Gate self-check: a deliberately allocating processor MUST be flagged.
    let self_check = benchmark_audio_block(BLOCK_SIZE_512, 8, |_buffer, _context| {
        let scratch = Vec::<f32>::with_capacity(128); // violation on purpose
        std::hint::black_box(&scratch);
    });
    if std::panic::catch_unwind(|| self_check.assert_real_time_safe()).is_err() {
        println!("self-check: allocating processor correctly flagged");
    } else {
        eprintln!("self-check: FAIL — the allocation gate did not fire");
        failures += 1;
    }

    if failures > 0 {
        panic!("real-time safety benchmark failed: {failures} gate(s) tripped");
    }
    println!("real-time safety benchmark passed: zero allocations on all callbacks");
}
