//! The real-time safety macros.
//!
//! - [`assert_real_time_safe!`](crate::assert_real_time_safe) — declarative
//!   macro that runs a block and fails the test the moment the block is
//!   observed to allocate.
//! - [`bench_real_time`] — re-export of the `tpt-av-test-macros` attribute
//!   that turns a whole function into the same kind of gate (with an optional
//!   wall-clock budget).
//!
//! [`bench_real_time`]: crate::bench_real_time

pub use tpt_av_test_macros::bench_real_time;

/// Asserts that a block of code is real-time safe: it completes without a
/// single heap allocation on the current thread.
///
/// ```rust
/// use tpt_av_test_benchmark::assert_real_time_safe;
///
/// fn mix_into(buffer: &mut [f32], gain: f32) {
///     for sample in buffer.iter_mut() {
///         *sample *= gain; // pure arithmetic: real-time safe
///     }
/// }
///
/// let mut buffer = [0.5f32; 512];
/// let peak = assert_real_time_safe!("mixer pass", {
///     mix_into(&mut buffer, 0.9);
///     buffer[0]
/// });
/// assert_eq!(peak, 0.45);
/// ```
///
/// The macro takes an optional label first (used in the failure message):
///
/// ```rust
/// tpt_av_test_benchmark::assert_real_time_safe!({ 2.0_f32.sqrt() });
/// ```
///
/// # Failure modes
///
/// - If the block allocates without panicking, the assertion fails with the
///   allocation count — that is a real-time safety violation and must fix the
///   code, not the test.
/// - If the block panics, the original panic is re-raised *unchanged* (its
///   message and backtrace are the real diagnosis; a panic already failed the
///   real-time budget). The panic machinery's own internal allocations are
///   deliberately not reported as violations.
/// - Memory allocated *before* the block (like the buffer above) and freed
///   inside it is fine — only new allocations count.
///
/// # Cost model
///
/// The block is wrapped in `catch_unwind`, so a panicking block fails the
/// test instead of aborting the binary. Counting is thread-local: allocations
/// on other threads are invisible here, matching how an audio callback
/// executes.
#[macro_export]
macro_rules! assert_real_time_safe {
    ($code:block) => {
        $crate::assert_real_time_safe!("real-time path", $code)
    };
    ($label:expr, $code:block) => {{
        let __tpt_tracker = $crate::allocation_tracker::AllocationTracker::new(true);
        let __tpt_result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| $code));
        match __tpt_result {
            ::std::result::Result::Ok(__tpt_value) => {
                let __tpt_allocations = __tpt_tracker.get_allocation_count();
                assert_eq!(
                    __tpt_allocations, 0,
                    "real-time safety violation in `{}`: {} heap allocation(s) detected",
                    $label, __tpt_allocations,
                );
                __tpt_value
            }
            // The original panic message and backtrace are already out;
            // re-raise the payload untouched.
            ::std::result::Result::Err(__tpt_payload) => ::std::panic::resume_unwind(__tpt_payload),
        }
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn macro_passes_allocation_free_blocks_and_yields_their_value() {
        let value = assert_real_time_safe!({ 40 + 2 });
        assert_eq!(value, 42);

        let mut buffer = [1.0f32; 512];
        let sum = assert_real_time_safe!("mix", {
            let mut total = 0.0f32;
            for sample in buffer.iter_mut() {
                *sample *= 0.5;
                total += *sample;
            }
            total
        });
        assert_eq!(sum, 256.0);
        let _ = buffer;
    }

    #[test]
    fn macro_fails_on_allocating_blocks() {
        let outcome = std::panic::catch_unwind(|| {
            let mut leaked = Vec::new();
            assert_real_time_safe!({
                leaked.push(1u8); // allocates inside the block
            });
            drop(leaked);
        });
        assert!(outcome.is_err(), "allocating block must fail");
    }

    #[test]
    fn macro_reraises_panics_unchanged() {
        let outcome = std::panic::catch_unwind(|| {
            assert_real_time_safe!({ panic!("callback exploded") });
        });
        let message = outcome
            .err()
            .and_then(|payload| {
                if let Some(text) = payload.downcast_ref::<&str>() {
                    Some((*text).to_string())
                } else {
                    payload.downcast_ref::<String>().cloned()
                }
            })
            .expect("macro must re-raise the original panic payload");
        assert_eq!(message, "callback exploded");
    }

    #[test]
    fn macro_prefers_the_original_panic_over_the_allocation_report() {
        let outcome = std::panic::catch_unwind(|| {
            assert_real_time_safe!({
                let _scratch = String::from("allocates");
                panic!("and then explodes");
            });
        });
        // The block failed either way, but the *panic* is the real diagnosis;
        // the allocation count must not mask it.
        let message = outcome
            .err()
            .and_then(|payload| {
                if let Some(text) = payload.downcast_ref::<&str>() {
                    Some((*text).to_string())
                } else {
                    payload.downcast_ref::<String>().cloned()
                }
            })
            .expect("expected the original panic to be re-raised");
        assert_eq!(message, "and then explodes");
    }
}
