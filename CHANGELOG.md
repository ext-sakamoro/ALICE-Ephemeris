# Changelog

All notable changes to this project are documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `Instant<S>` on the `Tai`, `Tt` and `Tdb` scales: `i64` seconds since
  J2000.0 plus attoseconds, checked integer arithmetic, `Duration`.
- UTC ↔ TAI (`UtcDateTime::to_tai`, `UtcDateTime::from_tai`,
  `tai_minus_utc`): the TAI − UTC table from 1960-01-01 (1960–1971 drift
  expressions in exact rationals, leap seconds through 2017-01-01, 23:59:60),
  valid until the table's expiry 2027-06-28; `ExpiryPolicy` and
  `Provenance` for dates past it.
- TAI ↔ TT (TT = TAI + 32.184 s).
- TT ↔ TDB with two models, `TdbModel::SpiceEt` and `TdbModel::Fb1990`;
  Fb1990 is defined within ±110,000 Julian years of J2000 TT
  (`tdb::FB1990_DOMAIN_MILLENNIA`) and returns NaN /
  `TimeError::OutOfRange` outside without evaluating the series. Inside it
  |TDB − TT| ≤ 0.920 s is proven from the coefficients (each sine at most 1,
  plus the f64 rounding bound), checked by a test; that bound reaches 1 s
  before 115,000 years, so a wider domain such as 120,000 years (where
  ERFA's value is still below 1 s) cannot be proven this way and is not used.
  The domain is part of the `TDB_FB1990` Law: its data and its reference
  grid (which includes both ends of the domain, half a second inside, and
  half a second and one second beyond).
  No earlier identifier was released, so no old → new entry is listed.
- Two-part Julian Date (`Jd2`) and exact Modified Julian Date (`Mjd`).
- `eop`: `EopRow::parse_finals2000a` reads IERS Rapid Service `finals2000A`
  rows (Bulletin A columns) as the integers they print; `EopTable::at` gives
  UT1 − TAI, the pole coordinates and dX, dY by 4-point Lagrange in the TAI
  time of the rows, with observed / predicted flags and the leap-table
  provenance; `Instant<Tai>::to_ut1` and `Instant<Ut1>::to_tai` (scale
  `Ut1`); `TimeError::EopOutOfRange` outside the rows.
- `frames`: Earth rotation angle (`era00`, `Instant<Ut1>::earth_rotation_angle`),
  GMST 1982 (`gmst82`), s′ (`sp00`), polar motion (`pom00`), TEME → ITRS
  (`TemeToItrs`), IAU 2006/2000A precession-nutation (`nut00a`, `nut06a`,
  `pfw06`, `obl06`, `pnm06a`), the CIO locator (`s06`) and the CIO-based
  matrices (`c2ixys`, `c2i06a`, `c2tcio`, `c2t06a`), in ERFA's operation order
  with alice-det-math's transcendentals.
- Laws `ut1-tai-eop`, `era-2000`, `teme-itrs-gmst82` and `iau2006-2000a-cio`
  in `law::ALL`.
- Laws with semantics identifiers (`law::ALL`).
- Reference tables generated from ERFA and CSPICE (`scripts/gen_time_golden.py`)
  and golden output digests checked on every CI platform and wasm32. The
  ERFA float columns are its expressions evaluated in IEEE binary64 with
  correctly rounded sines (a compiled ERFA may fuse a·b + c), so every table
  is reproduced bit for bit on arm64 and x86_64; pyerfa agrees within a
  derived bound.
- `scripts/mutants-equivalent.txt` with `scripts/mutants_equivalent_check.py`:
  the surviving mutants that cannot change a result, each with its reason;
  the check fails on an unlisted survivor, on a listed entry that no longer
  survives, and on an empty list, and reports timeouts as unverified.
- `scripts/public_text.sh`: the public text check on every tracked file and on
  the commits since the previous head; with no previous head (the first
  commit range of a branch) it reads the whole history, the first commit
  included.
