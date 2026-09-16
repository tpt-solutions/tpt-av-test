//! Integration tests: real-time safety gates as a consumer would use them,
//! including the `#[bench_real_time]` attribute composed with `#[test]`.

use core::time::Duration;

use tpt_av_test_benchmark::allocation_tracker::{count_allocations, AllocationTracker};
use tpt_av_test_benchmark::audio_block::benchmark_audio_block;
use tpt_av_test_benchmark::timing::{measure, parse_duration};

// ---------------------------------------------------------------------------
// #[bench_real_time] composed with #[test]
// ---------------------------------------------------------------------------

#[test]
#[tpt_av_test_benchmark::bench_real_time]
fn attribute_passes_for_allocation_free_bodies() {
    let mut buffer = [0.0f32; 1024];
    for sample in buffer.iter_mut() {
        *sample = sample.mul_add(0.5, 0.25);
    }
    assert_eq!(buffer[0], 0.25);
}

#[test]
#[tpt_av_test_benchmark::bench_real_time(max_duration = "10s", name = "budgeted-mixer")]
fn attribute_enforces_a_wall_clock_budget() {
    // 10 s budget, trivial body: must pass with room to spare.
    let mut total = 0u64;
    for value in 0..1024u64 {
        total = total.wrapping_add(value);
    }
    assert_eq!(total, 523_776);
}

#[test]
fn attribute_fails_allocating_bodies() {
    // Recreate what the attribute expands to (allocation + assert) since a
    // #[should_panic] fn cannot also carry a gate that must observe it.
    let outcome = std::panic::catch_unwind(|| {
        let tracker = AllocationTracker::new(true);
        let _scratch = String::from("allocates");
        let allocations = tracker.get_allocation_count();
        assert_eq!(
            allocations, 0,
            "real-time safety violation in `body`: {allocations} heap allocation(s) detected"
        );
    });
    assert!(outcome.is_err());
}

#[test]
fn parse_duration_supports_attribute_budget_syntax() {
    assert_eq!(
        parse_duration("11ms").unwrap(),
        Duration::from_millis(11),
        "the syntax used in #[bench_real_time(max_duration = \"...\")]"
    );
}

// ---------------------------------------------------------------------------
// count_allocations + AllocationTracker end-to-end
// ---------------------------------------------------------------------------

#[test]
fn count_allocations_reports_vec_growth() {
    let (len, stats) = count_allocations(|| {
        let mut buffer = Vec::with_capacity(1024);
        buffer.push(1.0f32);
        buffer.len()
    });
    assert_eq!(len, 1);
    assert_eq!(stats.allocations, 1);
    assert_eq!(stats.bytes_allocated, 1024 * 4);
}

#[test]
fn tracker_across_threads_is_invisible_to_this_thread() {
    let tracker = AllocationTracker::new(false);
    let handle = std::thread::spawn(|| {
        // Hundreds of allocations on the spawned thread.
        let junk: Vec<Vec<u8>> = (0..64).map(|_| vec![0u8; 1024]).collect();
        std::hint::black_box(&junk);
    });
    handle.join().unwrap();
    // `thread::spawn` itself boxes the closure and packet on THIS thread (a
    // handful of allocations), but none of the child thread's heap traffic
    // may leak into this thread's count.
    let count = tracker.get_allocation_count();
    assert!(
        count < 20,
        "counting must be thread-local (child thread allocated hundreds), got {count}"
    );
}

// ---------------------------------------------------------------------------
// audio block simulation end-to-end
// ---------------------------------------------------------------------------

#[test]
fn simulated_512_and_1024_callbacks_pass_together() {
    for block_size in [512usize, 1024usize] {
        let result = benchmark_audio_block(block_size, 16, |buffer, context| {
            assert_eq!(buffer.len(), context.block_size * context.channels);
            for sample in buffer.iter_mut() {
                *sample = sample.mul_add(0.5, 1.0);
            }
        });
        result.assert_real_time_safe();
        result.assert_within_block_budget();
    }
}

#[test]
fn budget_usage_is_a_fraction_below_one_for_fast_callbacks() {
    let result = benchmark_audio_block(512, 8, |_buffer, _context| {});
    assert!(
        result.budget_usage() < 1.0,
        "usage = {}",
        result.budget_usage()
    );
}

#[test]
fn measure_is_cheap_enough_for_tests() {
    let (value, elapsed) = measure(|| 6 * 7);
    assert_eq!(value, 42);
    assert!(elapsed < Duration::from_secs(1));
}
