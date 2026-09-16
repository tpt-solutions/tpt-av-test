//! Heap allocation tracking via a process-wide counting allocator.
//!
//! [`TrackingAllocator`] replaces the system allocator (the crate installs it
//! as its `#[global_allocator]`; every binary that links this crate inherits
//! it, which is exactly what we want for a dev-dependency-only harness). It
//! is a thin wrapper: when no [`AllocationTracker`] is active on the current
//! thread it only forwards to [`std::alloc::System`]; while a tracker is
//! active it additionally counts allocations, deallocations, and bytes for
//! that thread.
//!
//! Counting is thread-local, so a tracked scope observes exactly the thread
//! it was created on — the same guarantee an audio callback has. Aggregate
//! counters across all tracked threads are available via the
//! [`global_allocation_count`] family of functions for diagnostics.
//!
//! ```rust
//! use tpt_av_test_benchmark::allocation_tracker::{count_allocations, AllocationTracker};
//!
//! // Counting helper: returns the value plus what the heap did.
//! let (_value, stats) = count_allocations(|| {
//!     let mut buffer = Vec::new();          // allocates
//!     buffer.push(1u8);                     // may grow (realloc)
//!     buffer.len()
//! });
//! assert!(stats.allocations >= 1, "expected at least the Vec buffer alloc");
//!
//! // A scope with no heap activity must report zero.
//! let tracker = AllocationTracker::new(false);
//! let mut stack = [0.0f32; 512];
//! for sample in &mut stack {
//!     *sample *= 0.5;
//! }
//! assert_eq!(tracker.get_allocation_count(), 0);
//! ```
//!
//! # Strict mode
//!
//! `AllocationTracker::new(true)` arms *strict mode*: the tracker panics at
//! the end of its scope if any allocation was observed. Strictness is
//! enforced on drop (when not already unwinding) and via
//! [`AllocationTracker::assert_clean`]; the [`assert_real_time_safe!`] macro
//! and the `#[bench_real_time]` attribute are built on exactly this.
//!
//! The allocator itself never panics — panicking inside `GlobalAlloc` risks
//! aborting the process during unwinding. Violations are counted and surfaced
//! by the surrounding scope instead, which reports them as ordinary test or
//! bench failures.
//!
//! [`assert_real_time_safe!`]: crate::assert_real_time_safe

use std::alloc::System;
use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;

/// Process-wide allocator that counts heap activity on the current thread
/// while an [`AllocationTracker`] scope is active, and otherwise forwards
/// straight to [`std::alloc::System`].
///
/// By default (the `tracking-allocator` feature) this crate installs it as
/// the process `#[global_allocator]`, so simply depending on the crate makes
/// every allocation observable. A binary that needs its *own* allocator can
/// opt out with `default-features = false` and then declare
/// `static GLOBAL: tpt_av_test_benchmark::TrackingAllocator = ...` itself —
/// only one `#[global_allocator]` may exist per binary.
pub struct TrackingAllocator;

#[cfg(feature = "tracking-allocator")]
#[global_allocator]
static TRACKING_ALLOCATOR: TrackingAllocator = TrackingAllocator;

thread_local! {
    /// Per-thread counting state. `depth` is how many [`AllocationTracker`]s
    /// are active; counting happens only while it is non-zero, so normal
    /// (untracked) code pays nothing beyond a TLS read.
    static SCOPE: Scope = const {
        Scope {
            depth: Cell::new(0),
            allocations: Cell::new(0),
            deallocations: Cell::new(0),
            bytes_allocated: Cell::new(0),
            bytes_deallocated: Cell::new(0),
        }
    };
}

#[derive(Debug)]
struct Scope {
    depth: Cell<u64>,
    allocations: Cell<u64>,
    deallocations: Cell<u64>,
    bytes_allocated: Cell<u64>,
    bytes_deallocated: Cell<u64>,
}

/// Aggregate counters across every tracked thread.
static GLOBAL_ALLOCATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GLOBAL_DEALLOCATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GLOBAL_BYTES_ALLOCATED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GLOBAL_BYTES_DEALLOCATED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Heap activity observed between the creation of an [`AllocationTracker`]
/// and the moment the stats were read.
///
/// A `realloc` is accounted as one deallocation plus one allocation, because
/// in a real-time callback growing a buffer *is* an allocation violation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllocationStats {
    /// Number of heap allocations.
    pub allocations: u64,
    /// Number of heap deallocations.
    pub deallocations: u64,
    /// Total bytes handed out by the allocator.
    pub bytes_allocated: u64,
    /// Total bytes returned to the allocator.
    pub bytes_deallocated: u64,
}

impl AllocationStats {
    fn thread_current() -> Self {
        // `try_with` (not `with`): the allocator may be invoked while this
        // thread's TLS is being torn down, where `with` would panic.
        SCOPE
            .try_with(|scope| AllocationStats {
                allocations: scope.allocations.get(),
                deallocations: scope.deallocations.get(),
                bytes_allocated: scope.bytes_allocated.get(),
                bytes_deallocated: scope.bytes_deallocated.get(),
            })
            .unwrap_or_default()
    }

    /// Difference between two snapshots, saturating at zero (memory
    /// allocated before the snapshot may be freed inside the scope).
    fn saturating_sub(self, earlier: Self) -> Self {
        AllocationStats {
            allocations: self.allocations.saturating_sub(earlier.allocations),
            deallocations: self.deallocations.saturating_sub(earlier.deallocations),
            bytes_allocated: self.bytes_allocated.saturating_sub(earlier.bytes_allocated),
            bytes_deallocated: self
                .bytes_deallocated
                .saturating_sub(earlier.bytes_deallocated),
        }
    }
}

impl fmt::Display for AllocationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} allocation(s) / {} byte(s) allocated, {} deallocation(s) / {} byte(s) freed",
            self.allocations, self.bytes_allocated, self.deallocations, self.bytes_deallocated
        )
    }
}

/// RAII guard that counts every heap allocation on the current thread between
/// its creation and its last read.
///
/// ```rust
/// use tpt_av_test_benchmark::allocation_tracker::AllocationTracker;
///
/// let tracker = AllocationTracker::new(true); // strict: drop panics on alloc
/// assert_eq!(tracker.get_allocation_count(), 0);
/// ```
///
/// # Nesting
///
/// Each tracker snapshots the thread counters at creation, so nested scopes
/// report only what happened inside themselves, while the outermost scope
/// still sees everything on its thread. The guard is deliberately `!Send`:
/// the counters it reads live in this thread's TLS.
pub struct AllocationTracker {
    snapshot: AllocationStats,
    strict: bool,
    _not_send: PhantomData<*mut ()>,
}

impl AllocationTracker {
    /// Starts counting heap activity on the current thread.
    ///
    /// With `panics_on_alloc = true` the tracker is *strict*: it panics when
    /// dropped if any allocation was observed (and when already unwinding, it
    /// stays quiet so it cannot turn a panic into an abort). Use
    /// [`AllocationTracker::new(false)`] for plain measurement.
    pub fn new(panics_on_alloc: bool) -> Self {
        SCOPE.with(|scope| scope.depth.set(scope.depth.get() + 1));
        AllocationTracker {
            snapshot: AllocationStats::thread_current(),
            strict: panics_on_alloc,
            _not_send: PhantomData,
        }
    }

    /// Number of heap allocations observed since this tracker was created.
    pub fn get_allocation_count(&self) -> usize {
        usize::try_from(self.allocation_stats().allocations).unwrap_or(usize::MAX)
    }

    /// Full allocation statistics observed since this tracker was created.
    pub fn allocation_stats(&self) -> AllocationStats {
        AllocationStats::thread_current().saturating_sub(self.snapshot)
    }

    /// Panics if any allocation was observed since creation. `context` names
    /// the real-time path under test in the failure message.
    pub fn assert_clean(&self, context: &str) {
        let stats = self.allocation_stats();
        assert_eq!(
            stats.allocations, 0,
            "real-time safety violation in `{context}`: {stats}",
        );
    }

    /// Whether this tracker panics on observed allocations (see
    /// [`AllocationTracker::new`]).
    pub fn is_strict(&self) -> bool {
        self.strict
    }
}

impl Drop for AllocationTracker {
    fn drop(&mut self) {
        let stats = self.allocation_stats();
        SCOPE.with(|scope| scope.depth.set(scope.depth.get().saturating_sub(1)));
        if self.strict && stats.allocations > 0 && !std::thread::panicking() {
            panic!("real-time safety violation: {stats} inside a strict AllocationTracker scope");
        }
    }
}

/// Runs `f` with an (inactive, non-strict) tracking scope and returns both its
/// result and the heap activity it caused on the current thread.
pub fn count_allocations<F, R>(f: F) -> (R, AllocationStats)
where
    F: FnOnce() -> R,
{
    let tracker = AllocationTracker::new(false);
    let result = f();
    let stats = tracker.allocation_stats();
    (result, stats)
}

/// Total heap allocations counted across all tracked threads since process
/// start.
pub fn global_allocation_count() -> u64 {
    GLOBAL_ALLOCATIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Total heap deallocations counted across all tracked threads since process
/// start.
pub fn global_deallocation_count() -> u64 {
    GLOBAL_DEALLOCATIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Total bytes allocated across all tracked threads since process start.
pub fn global_bytes_allocated() -> u64 {
    GLOBAL_BYTES_ALLOCATED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Total bytes freed across all tracked threads since process start.
pub fn global_bytes_deallocated() -> u64 {
    GLOBAL_BYTES_DEALLOCATED.load(std::sync::atomic::Ordering::Relaxed)
}

fn record_alloc(size: usize) {
    let _ = SCOPE.try_with(|scope| {
        if scope.depth.get() == 0 {
            return;
        }
        scope.allocations.set(scope.allocations.get() + 1);
        scope
            .bytes_allocated
            .set(scope.bytes_allocated.get() + size as u64);
        GLOBAL_ALLOCATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        GLOBAL_BYTES_ALLOCATED.fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
    });
}

fn record_dealloc(size: usize) {
    let _ = SCOPE.try_with(|scope| {
        if scope.depth.get() == 0 {
            return;
        }
        scope.deallocations.set(scope.deallocations.get() + 1);
        scope
            .bytes_deallocated
            .set(scope.bytes_deallocated.get() + size as u64);
        GLOBAL_DEALLOCATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        GLOBAL_BYTES_DEALLOCATED.fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
    });
}

// SAFETY: every method forwards to `std::alloc::System` with the same
// arguments and only touches atomics/thread-local counters around the call,
// so all `GlobalAlloc` invariants are inherited from `System`.
unsafe impl std::alloc::GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() && layout.size() > 0 {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        System.dealloc(ptr, layout);
        if layout.size() > 0 {
            record_dealloc(layout.size());
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if new_ptr.is_null() {
            return new_ptr;
        }
        if layout.size() > 0 {
            record_dealloc(layout.size());
        }
        if new_size > 0 {
            record_alloc(new_size);
        }
        new_ptr
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() && layout.size() > 0 {
            record_alloc(layout.size());
        }
        ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_a_box_allocation() {
        let (len, stats) = count_allocations(|| {
            let boxed = Box::new([0u8; 128]);
            boxed.len() // the box is dropped inside the scope
        });
        assert_eq!(len, 128);
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.bytes_allocated, 128);
        assert_eq!(stats.deallocations, 1);
    }

    #[test]
    fn clean_stack_only_code_is_free() {
        let tracker = AllocationTracker::new(true);
        let mut buffer = [0.0f32; 512];
        for sample in &mut buffer {
            *sample += 1.0;
        }
        assert_eq!(tracker.get_allocation_count(), 0);
        assert_eq!(tracker.allocation_stats().bytes_allocated, 0);
    }

    #[test]
    fn nested_trackers_see_only_their_own_region() {
        let outer = AllocationTracker::new(false);
        let leaked = Box::new(0u32); // counts toward `outer`
        let inner = AllocationTracker::new(false);
        let inner_alloc = Box::new(1u32);
        assert_eq!(inner.get_allocation_count(), 1);
        drop(inner_alloc);
        drop(inner);
        assert!(
            outer.get_allocation_count() >= 2,
            "outer scope must see allocations from the inner region too"
        );
        drop(leaked);
        drop(outer);
    }

    #[test]
    fn strict_tracker_panics_on_drop_after_allocation() {
        let outcome = std::panic::catch_unwind(|| {
            let tracker = AllocationTracker::new(true);
            let _garbage = String::from("long enough to defeat small-string optimization!!!");
            // Drop inside the closure so the panic is caught here, not
            // outside `catch_unwind`.
            drop(tracker);
        });
        assert!(outcome.is_err(), "strict tracker must panic on allocation");
    }

    #[test]
    fn strict_tracker_is_quiet_when_clean() {
        let outcome = std::panic::catch_unwind(|| {
            let tracker = AllocationTracker::new(true);
            assert_eq!(tracker.get_allocation_count(), 0);
        });
        assert!(outcome.is_ok());
    }

    #[test]
    fn realloc_is_counted_as_alloc_plus_dealloc() {
        let tracker = AllocationTracker::new(false);
        let mut vector = Vec::with_capacity(4);
        vector.push(1u8);
        vector.reserve(4096); // guaranteed realloc
        let stats = tracker.allocation_stats();
        assert!(stats.allocations >= 1);
        assert!(
            stats.deallocations >= 1,
            "realloc must count as a dealloc + alloc"
        );
    }

    #[test]
    fn global_counters_track_other_threads() {
        let before = global_allocation_count();
        let handle = std::thread::spawn(|| {
            let tracker = AllocationTracker::new(false);
            // One genuine heap allocation (with_capacity survives clippy's
            // useless-vec lint because the allocation is the point).
            let _heap: Vec<u8> = Vec::with_capacity(64);
            assert_eq!(tracker.get_allocation_count(), 1);
        });
        handle.join().unwrap();
        assert!(
            global_allocation_count() > before,
            "allocations on other threads must reach the global counters"
        );
    }

    #[test]
    fn stats_display_is_informative() {
        let stats = AllocationStats {
            allocations: 2,
            bytes_allocated: 96,
            deallocations: 1,
            bytes_deallocated: 64,
        };
        let text = stats.to_string();
        assert!(text.contains("2 allocation(s)"), "{text}");
        assert!(text.contains("96 byte(s)"), "{text}");
    }
}
