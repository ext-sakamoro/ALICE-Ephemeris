# ALICE-Ephemeris

Deterministic astronomical time scales for Rust: UTC, TAI, TT and TDB, leap
seconds and Julian dates. Every conversion returns the same bits on every
target (Linux, macOS, Windows, wasm32), and is checked against independent
references (ERFA and NASA NAIF SPICE).

It is the time layer for ephemerides and reference frames that follow.

**Not for:** UT1 and Earth orientation (needs IERS EOP data, not included),
topocentric TDB terms (≤ 2 µs), TCG / TCB, local time zones, or reading the
system clock. Planetary positions other than the Earth and the Moon are not
part of this release.

## What it does

| Conversion | How | Agreement with the reference |
|---|---|---|
| UTC ↔ TAI | TAI − UTC table from 1960-01-01 (1960–1971 drift expressions, then every leap second, 23:59:60) in exact integer attoseconds | ERFA `eraUtctai` / `eraTaiutc` to its own rounding (≤ 5 × 10⁻¹¹ s); integer offsets from 1972 exact |
| TAI ↔ TT | TT = TAI + 32.184 s | exact |
| TT ↔ TDB, `SpiceEt` | SPICE `DELTET` formula (`naif0012.tls`) | CSPICE `unitim` within 2 ulp of its `f64` result |
| TT ↔ TDB, `Fb1990` | Fairhead & Bretagnon (1990), 787 terms, geocentric | ERFA `eraDtdb` within 10⁻¹⁵ s; model accuracy ±3 ns for 1950–2050 (as stated by ERFA) |
| Instant ↔ JD / MJD | two-part JD (`jd1` + `jd2`) for interoperability, exact MJD parts | exact rational fraction within 1 ulp |
| TAI ↔ UT1, polar motion (`eop`) | rows of an IERS `finals2000A` table (the caller's, as printed integers); UT1 − TAI interpolated by 4-point Lagrange in the TAI time of the rows (smooth across leap seconds); observed / predicted flags; no extrapolation | the same interpolation in exact rationals within 10⁻¹⁵ s and 10⁻¹⁵″ |
| Earth rotation angle, GMST (1982), s′ (`frames`) | ERFA `era00` / `gmst82` / `sp00` in the same IEEE operations | bit for bit with ERFA's expressions evaluated in IEEE binary64 |
| TEME → ITRS (`frames::TemeToItrs`) | GMST (1982) and polar motion, velocity with the length-of-day term (Vallado et al. 2006) | ERFA composition within 10⁻¹¹ km; AIAA 2006-6753 example within its one-part Julian date |
| GCRS → CIRS → ITRS (`frames::c2i06a`, `c2t06a`) | IAU 2006/2000A precession-nutation (1365 nutation terms), CIO locator s, ERA, polar motion | ERFA test vectors; pyerfa 1800–2200 within 2 × 10⁻¹⁶ rad (nutation), 10⁻¹⁵ (matrices) |
| Earth and Sun (`sunmoon::epv00`) | ERFA `eraEpv00`: simplified VSOP2000 (1,951 terms), heliocentric and barycentric position and velocity, BCRS | ERFA's expressions within a derived bound (≤ 4 × 10⁻¹³ au); JPL Horizons DE441 1900–2100 within the model's 11.2 km |
| Moon (`sunmoon::moon98`) | ERFA `eraMoon98`: Meeus (1998) series, 120 terms, rotated into the GCRS by IAU 2006 precession | ERFA's expressions within a derived bound (≤ 2 × 10⁻¹⁶ au); Meeus Example 47.a; JPL Horizons DE441 within the model's 31.7 km |
| Sun and Moon in a site's sky (`horizon::sky`, `moon_phase`) | geometric topocentric altitude and azimuth (WGS84 site, IAU 2006/2000A celestial-to-terrestrial matrix with the caller's UT1 and pole), the Moon's phase angle, illuminated fraction and waxing | JPL Horizons observer tables at four sites: with light time and aberration added, within the models' accuracy plus 0.6″·sec φ (the pole convention); as it is, within that plus the aberration it leaves out (≤ 21.2″); ERFA's `gd2gc` test vector |

```rust
use alice_ephemeris::{ExpiryPolicy, TdbModel, UtcDateTime};

let utc = UtcDateTime::new(2016, 12, 31, 23, 59, 60, 0)?; // a leap second
let tai = utc.to_tai(ExpiryPolicy::Strict)?.value;
let tt = tai.to_tt()?;
let tdb = tt.to_tdb(TdbModel::Fb1990)?; // Instant<Tdb>
assert_eq!(UtcDateTime::from_tai(tai, ExpiryPolicy::Strict)?.value, utc);
assert!(TdbModel::Fb1990.tdb_minus_tt(tt).abs() < 2e-3);
assert_eq!(tdb.to_tt(TdbModel::Fb1990)?, tt);
# Ok::<(), alice_ephemeris::TimeError>(())
```

## Representation and guaranteed ranges

- An `Instant<S>` (`S` = `Tai`, `Tt` or `Tdb`) is `i64` seconds since J2000.0
  (2000-01-01 12:00:00 on that scale) plus `u64` attoseconds. Range ±2⁶³ s
  (about ±2.9 × 10¹¹ years) at a resolution of exactly 10⁻¹⁸ s. Addition and
  subtraction are integer, never round, and return `TimeError::OutOfRange`
  instead of wrapping.
- UTC is a label of TAI, held as a civil date and time (`UtcDateTime`).
  Defined from **1960-01-01T00:00:00 UTC** up to, not including, the
  leap-second table's expiry, **2027-06-28T00:00:00 UTC**. Before 1960 every
  conversion returns `TimeError::BeforeUtc`.
- TDB − TT is evaluated in `f64`. `TdbModel::SpiceEt` evaluates for any
  instant; `TdbModel::Fb1990` is defined within ±110,000 Julian years of
  J2000 TT (`tdb::FB1990_DOMAIN_MILLENNIA`), where |TDB − TT| ≤ 0.920 s is
  proven from its coefficients, and its conversions return
  `TimeError::OutOfRange` outside. Fb1990 carries ERFA's ±3 ns statement
  only for 1950–2050 TT (`TdbModel::within_stated_accuracy`); the two
  models differ by at most 38 µs over that period (measured).
- `to_seconds_f64` is the nearest `f64` for whole seconds and within 1 ulp
  otherwise (any `i64` second); `to_jd2` is within 1 ulp of the exact value;
  `from_jd2` is as precise as its `f64` inputs (≈ 10 ps for a day fraction).

## Leap-second table expiry

The table is the IANA `leap-seconds.list` (updated through IERS Bulletin C,
file SHA-1 checked by the tests), which expires on **2027-06-28**. On and after
that date a leap second may have been announced that this version does not
know:

- `ExpiryPolicy::Strict` (default) returns `TimeError::LeapTableExpired`.
- `ExpiryPolicy::AssumeNoNewLeapSeconds` keeps TAI − UTC = 37 s and marks the
  result `Provenance::AssumedNoNewLeapSeconds`.

A new table is a new release with a new semantics identifier for UTC ↔ TAI.

## Determinism

<!-- claim-test: golden_digests -->

The only `f64` arithmetic is the TDB − TT series and the Julian-date
fraction. Every transcendental function there is `alice-det-math`'s (IEEE
basic operations only); `clippy.toml` forbids the platform `f64::sin` and
friends, `mul_add`, the wall clock, threads and hash-ordered collections.
`tests/law_ids.rs` hashes the outputs of every conversion over fixed grids;
CI asserts the same hashes on Linux (x86_64, aarch64), macOS (aarch64,
x86_64), Windows and wasm32-wasip1.

## MSRV

Rust 1.85 (`rust-version` in `Cargo.toml`), checked by the CI `msrv` job.

## Laws

Each conversion is a Law with a 32-byte semantics identifier (`law::ALL`):
`utc-tai`, `tt-tai`, `tdb-spice-et`, `tdb-fb1990`, `julian-date`,
`ut1-tai-eop`, `era-2000`, `teme-itrs-gmst82`, `iau2006-2000a-cio`,
`earth-epv00`, `moon98`, `sun-moon-horizon`. The
identifier is the SHA-256 of the Law's name, tables and constants (as bit
patterns), the `alice-det-math` semantics identifier where it is used, and
the digest of its outputs over a frozen reference grid. It moves if and only
if those inputs or the reference outputs move; growing the regression tests
does not move it.

## References and test oracles

None of the expected values in `tests/golden/` comes from this crate.
`scripts/gen_time_golden.py` (versions pinned in
`scripts/oracle-requirements.txt`) writes them from:

- ERFA 2.0.1: `eraDtf2d`, `eraUtctai` and `eraDtdb` as their own
  expressions evaluated in IEEE binary64 (sines correctly rounded), with
  pyerfa run on every row and agreeing within a bound derived from the
  arithmetic; `eraTaiutc`, `eraD2dtf` and `eraCal2jd` through pyerfa
  (integer results). A compiled ERFA may fuse a·b + c, which changes the
  last bits, so the tables are the same on every machine only this way:
  `scripts/oracle_check.sh` reproduces them bit for bit on arm64 and
  x86_64;
- CSPICE N0067 through spiceypy with `naif0012.tls`: `unitim`;
- exact rational arithmetic for Julian-date fractions;
- `scripts/gen_eop_golden.py`: an excerpt of the USNO `finals2000A.all`
  (2026-10-23, SHA-256 of the whole file in `tests/oracle_eop.rs`)
  interpolated in exact rationals with its own column reading;
- `scripts/gen_frames_golden.py` and `scripts/gen_cio_golden.py`: ERFA's
  angle and argument expressions evaluated in IEEE binary64 (bit reference),
  pyerfa for the series and matrices (within derived bounds), mpmath for the
  Earth rotation angle of exact instants, and the ERFA test vectors of
  `t_erfa_c.c`. The IAU 2000A and s series are generated from ERFA `nut00a.c`
  and `s06.c` by `scripts/gen_iau2006_tables.py` (SHA-256 pinned);
- `scripts/gen_sunmoon_golden.py`: ERFA `eraEpv00` and `eraMoon98` evaluated
  in IEEE binary64 (bit reference, with a bound per row for 1-ulp sines),
  pyerfa within derived bounds; the coefficient tables are generated from
  ERFA `epv00.c` and `moon98.c` by `scripts/gen_sunmoon_tables.py` (SHA-256
  pinned). Independent of ERFA: JPL Horizons DE441 states
  (`tests/data/horizons-sunmoon.csv`, `scripts/fetch_horizons_sunmoon.py`)
  and Meeus, *Astronomical Algorithms*, Example 47.a;
- JPL Horizons observer tables (`tests/data/horizons-sky.csv`,
  `scripts/fetch_horizons_sky.py`) for the sky: airless apparent azimuth and
  elevation at four sites and the Moon's phase, at instants inside the IERS
  rows of the EOP excerpt.

The 1960–1971 TAI − UTC rows are those of ERFA `dat.c` (USNO
`tai-utc.dat`); the Fairhead & Bretagnon coefficients are generated from
ERFA `dtdb.c` by `scripts/gen_fb1990_table.py` (source file SHA-256 pinned).

## License

Copyright (c) 2026 Moroya Sakamoto. All rights reserved; no license is
granted (see `LICENSE`). Third-party components keep their own licenses
(ERFA BSD-3-Clause, alice-det-math Apache-2.0, the leap-second file public
domain); see `THIRD-PARTY-NOTICES`.
