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
  Fb1990 is defined within ±130,000 Julian years of J2000 TT
  (`tdb::FB1990_DOMAIN_MILLENNIA`, where |TDB − TT| < 1 s) and returns
  NaN / `TimeError::OutOfRange` outside without evaluating the series.
  The domain is part of the `TDB_FB1990` Law: its data and its reference
  grid (which includes both ends of the domain and one second beyond).
  No earlier identifier was released, so no old → new entry is listed.
- Two-part Julian Date (`Jd2`) and exact Modified Julian Date (`Mjd`).
- Laws with semantics identifiers (`law::ALL`).
- Reference tables generated from ERFA and CSPICE (`scripts/gen_time_golden.py`)
  and golden output digests checked on every CI platform and wasm32.
- `scripts/mutants-equivalent.txt` with `scripts/mutants_equivalent_check.py`:
  the surviving mutants that cannot change a result, each with its reason;
  the check fails on an unlisted survivor, on a listed entry that no longer
  survives, and on an empty list, and reports timeouts as unverified.
