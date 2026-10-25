# Changelog

All notable changes to this project are documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- **Breaking:** 破壊的変更: `sunmoon::moon98` は `Pv` でなく `MoonPv { geocentric, in_range }` を返す `in_range` は日付が ERFA の精度の範囲 (ERFA の注 3 の ELP/MPP02 との比較の 1950–2100、J2000.0 から T が −0.5 から +1.0 Julian 世紀) にあるか (ERFA の routine は状態を返さない) 位置と速度の値と `law::MOON98` の識別子は変わらない
- `alice-det-math` を git rev `6384856c` (0.5.2) で引く
- manifest の authors から address を外した

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
- `sunmoon`: the Earth's heliocentric and barycentric position and velocity
  (`epv00`, ERFA `eraEpv00`, simplified VSOP2000) with the geocentric Sun
  (`EarthPv::sun_geocentric`), and the Moon's geocentric position and velocity
  (`moon98`, ERFA `eraMoon98`, Meeus 1998 rotated into the GCRS), in ERFA's
  operation order with alice-det-math's sines and cosines. ERFA's own
  departures are kept: the Moon's mean longitude omits the light-time
  correction (0.744″ from Meeus), its velocity neglects the spin of the
  precessing ecliptic (≤ 3 mm/s), and the rate of Meeus's argument A₁ is
  taken as 0.003958°/century (≤ 2.2 mm/s). Laws `earth-epv00` and `moon98` in
  `law::ALL`. Checked against ERFA's expressions in IEEE binary64 (bounds
  derived per row), pyerfa, JPL Horizons DE441 and Meeus Example 47.a.
- `horizon`: the Sun and the Moon in a site's sky (`sky`): geometric
  topocentric altitude, azimuth and distance on the WGS84 ellipsoid (`Site`,
  `Site::itrs` as ERFA `eraGd2gce`) through `frames::c2t06a` with the caller's
  UT1 and pole coordinates, so the Moon's parallax is included; the Moon's
  geocentric phase angle, illuminated fraction and waxing (`moon_phase`). No
  light time, aberration or refraction. Law `sun-moon-horizon` in `law::ALL`
  (its data holds the identifiers of `earth-epv00`, `moon98` and
  `iau2006-2000a-cio`). Checked against JPL Horizons observer tables at four
  sites, reached through UTC → TAI → TT and the IERS rows of the EOP excerpt.
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
