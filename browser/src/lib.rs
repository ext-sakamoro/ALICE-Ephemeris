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
    check_eo_reference_grids_are_frozen => law_ids_earth_orientation::reference_grids_are_frozen_check,
    check_eo_reference_digests => law_ids_earth_orientation::reference_digests_check,
    check_eo_semantics_ids => law_ids_earth_orientation::semantics_ids_check,
    check_sunmoon_reference_grid_is_frozen => law_ids_sunmoon::reference_grid_is_frozen_check,
    check_sunmoon_reference_digests => law_ids_sunmoon::reference_digests_check,
    check_sunmoon_semantics_ids => law_ids_sunmoon::semantics_ids_check,
    check_sunmoon_bench_sky_digest => law_ids_sunmoon::bench_sky_digest_check,
}

/// The `alice_det_math::SEMANTICS_ID` this build computes with (32 bytes at the returned
/// pointer), printed by the runner so a log says which arithmetic the browser build ran.
#[no_mangle]
pub extern "C" fn det_math_semantics_id() -> *const u8 {
    alice_det_math::SEMANTICS_ID.as_ptr()
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
