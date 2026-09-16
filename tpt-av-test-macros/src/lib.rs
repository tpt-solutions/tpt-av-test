//! Procedural macros backing the `tpt-av-test` benchmark harness.
//!
//! Currently provides [`bench_real_time`], an attribute macro that turns a
//! test or benchmark function into a real-time safety gate: the body runs
//! under `tpt-av-test-benchmark`'s allocation tracker and, optionally, a
//! wall-clock budget, and the function fails the moment a heap allocation (or
//! budget overrun) is detected.
//!
//! ```ignore
//! // In tpt-audio's test suite (needs both crates as dev-dependencies):
//! #[test]
//! #[tpt_av_test_macros::bench_real_time(max_duration = "11ms")]
//! fn mixer_512_sample_callback_is_rt_safe() {
//!     let mut buffer = vec![0.0f32; 512 * 2];
//!     for sample in &mut buffer {
//!         *sample = *sample * 0.5 + 0.25; // pretend mixing
//!     }
//! }
//! ```
//!
//! Compilable examples live in the `tpt-av-test-benchmark` crate docs, which
//! has both crates in scope.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Ident, ItemFn, LitStr, Token};

/// Marks a function as a real-time benchmark: the body must complete without
/// a single heap allocation, and (when configured) within a wall-clock
/// budget.
///
/// Any other attributes on the function (such as `#[test]` or `#[ignore]`)
/// are preserved, so the macro composes with the normal test harness:
///
/// ```ignore
/// #[test]
/// #[tpt_av_test_macros::bench_real_time]
/// fn no_allocations_allowed_here() {
///     let mut buffer = [0.0f32; 512];
///     for sample in &mut buffer {
///         *sample *= 0.5; // pure arithmetic: passes
///     }
///     assert_eq!(buffer[0], 0.0);
/// }
/// ```
///
/// # Options
///
/// - `max_duration = "<duration>"` — additionally assert that the body
///   finishes within the given duration (e.g. `"10ms"`, `"512us"`; syntax as
///   accepted by `tpt_av_test_benchmark::timing::parse_duration`).
/// - `name = "<label>"` — label used in failure messages (defaults to the
///   function name).
///
/// # Panicking code
///
/// A `?` or `return` that leaves the function early skips the safety
/// assertions (they run only on normal completion); in a `#[test]` fn the
/// early exit itself is already a failure, so nothing is lost.
#[proc_macro_attribute]
pub fn bench_real_time(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as BenchRealTimeArgs);
    let func = parse_macro_input!(item as ItemFn);

    let BenchRealTimeArgs { max_duration, name } = args;

    let attrs = &func.attrs;
    let vis = &func.vis;
    let sig = &func.sig;
    let body = &func.block;
    let label = match &name {
        Some(label) => quote!(#label),
        None => quote!(stringify!(#sig)),
    };

    let budget_check = match &max_duration {
        Some(literal) => quote! {
            let __tpt_budget: ::core::time::Duration =
                ::tpt_av_test_benchmark::timing::parse_duration(#literal)
                    .expect("invalid `max_duration` in #[bench_real_time]");
            assert!(
                __tpt_elapsed <= __tpt_budget,
                "real-time budget exceeded in `{}`: {:?} elapsed > {:?} budget",
                #label,
                __tpt_elapsed,
                __tpt_budget,
            );
        },
        None => quote! {},
    };

    let expanded = quote! {
        #(#attrs)*
        #vis #sig {
            let __tpt_tracker = ::tpt_av_test_benchmark::allocation_tracker::AllocationTracker::new(true);
            let __tpt_start = ::std::time::Instant::now();
            let __tpt_result = #body;
            let __tpt_elapsed = __tpt_start.elapsed();
            #budget_check
            let __tpt_allocations = __tpt_tracker.get_allocation_count();
            assert_eq!(
                __tpt_allocations, 0,
                "real-time safety violation in `{}`: {} heap allocation(s) detected",
                #label,
                __tpt_allocations,
            );
            __tpt_result
        }
    };

    expanded.into()
}

/// Parsed `#[bench_real_time(...)]` options.
#[derive(Default)]
struct BenchRealTimeArgs {
    max_duration: Option<LitStr>,
    name: Option<LitStr>,
}

impl Parse for BenchRealTimeArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = BenchRealTimeArgs::default();
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "max_duration" => args.max_duration = Some(value),
                "name" => args.name = Some(value),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown option `{other}`, expected `max_duration` or `name`",),
                    ));
                }
            }
            if input.parse::<Token![,]>().is_err() {
                break;
            }
        }
        Ok(args)
    }
}
