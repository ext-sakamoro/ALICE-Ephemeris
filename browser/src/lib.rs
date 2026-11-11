//! The Law identifier checks of `tests/law_ids*.rs`, compiled for
//! `wasm32-unknown-unknown` — the target a browser loads — and run under node
//! by `scripts/run_browser_wasm.sh`.
//!
//! The wasip1 lane runs every integration test, but a browser build is a
//! different target (no WASI, a different std); the crate promises the same
//! bits there too, so the same checks run there. The test files are included
//! as modules, so the grids and the pinned digests have one source; their
//! hashing is [`law_digest`], which the native runs use as well.
//!
//! Every export named `check_*` runs one check and returns 1; a failed check
//! panics, which traps. `check_count` is the number of `check_*` exports, so
//! the runner can refuse a run that skipped one.
//!
//! Author: Moroya Sakamoto

#![allow(clippy::missing_safety_doc)]

// the included test files name this crate as `alice_ephemeris_browser`, as the native test
// runs do (they reach it as a dev-dependency)
extern crate self as alice_ephemeris_browser;

pub mod law_digest;

#[path = "../../tests/law_ids.rs"]
mod law_ids;
#[path = "../../tests/law_ids_earth_orientation.rs"]
mod law_ids_earth_orientation;
#[path = "../../tests/law_ids_sunmoon.rs"]
mod law_ids_sunmoon;

use alice_ephemeris::horizon::sky;

macro_rules! checks {
    ($($export:ident => $f:path),* $(,)?) => {
        $(
            #[no_mangle]
            pub extern "C" fn $export() -> u32 {
                $f();
                1
            }
        )*
        /// The number of `check_*` exports.
        #[no_mangle]
        pub extern "C" fn check_count() -> u32 {
            [$(stringify!($export)),*].len() as u32
        }
    };
}

checks! {
    check_time_reference_grids_are_frozen => law_ids::reference_grids_are_frozen_check,
    check_time_reference_digests => law_ids::reference_digests_check,
    check_time_test_grid_digests => law_ids::test_grid_digests_check,
    check_time_semantics_ids => law_ids::semantics_ids_check,
    check_time_det_math_entries => law_ids::every_det_math_entry_is_the_resolved_function_check,
    check_time_earlier_identifiers => law_ids::the_earlier_identifiers_name_their_laws_and_only_those_check,
    check_eo_reference_grids_are_frozen => law_ids_earth_orientation::reference_grids_are_frozen_check,
    check_eo_reference_digests => law_ids_earth_orientation::reference_digests_check,
    check_eo_semantics_ids => law_ids_earth_orientation::semantics_ids_check,
    check_sunmoon_reference_grid_is_frozen => law_ids_sunmoon::reference_grid_is_frozen_check,
    check_sunmoon_reference_digests => law_ids_sunmoon::reference_digests_check,
    check_sunmoon_semantics_ids => law_ids_sunmoon::semantics_ids_check,
    check_sunmoon_bench_sky_digest => law_ids_sunmoon::bench_sky_digest_check,
    check_sunmoon_unix_sky_digest => law_ids_sunmoon::unix_sky_digest_check,
}

/// The number of Laws of the crate (`alice_ephemeris::law::ALL`), for `law_semantics_id`.
#[no_mangle]
pub extern "C" fn law_count() -> u32 {
    alice_ephemeris::law::ALL.len() as u32
}

/// The semantics identifier of the `i`-th Law of `alice_ephemeris::law::ALL` (32 bytes at
/// the returned pointer, null past the end), printed by the runner so a log says which
/// Laws the browser build computed: each identifier folds the identifiers of the
/// alice-det-math functions the crate calls (`law::DET_MATH_PINS`).
#[no_mangle]
pub extern "C" fn law_semantics_id(i: u32) -> *const u8 {
    match alice_ephemeris::law::ALL.get(i as usize) {
        Some(law) => law.semantics_id.as_ptr(),
        None => core::ptr::null(),
    }
}

/// Cost probe for the runner (reported, not gated): `n` evaluations of the
/// Sun and Moon at the inputs of `bench_sky_digest` (tests/law_ids_sunmoon.rs,
/// whose digest the `check_sunmoon_bench_sky_digest` export asserts). Returns
/// the sum of the Sun's altitudes so the work cannot be optimised away.
#[no_mangle]
pub extern "C" fn bench_sky(n: u32) -> f64 {
    let mut acc = 0.0;
    for i in 0..n {
        let (tt, ut1, site) = law_ids_sunmoon::bench_sky_input(i);
        acc += sky(tt, ut1, 0.0, 0.0, &site).sun.altitude;
    }
    acc
}
