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
- TT ↔ TDB with two models, `TdbModel::SpiceEt` and `TdbModel::Fb1990`.
- Two-part Julian Date (`Jd2`) and exact Modified Julian Date (`Mjd`).
- Laws with semantics identifiers (`law::ALL`).
- Reference tables generated from ERFA and CSPICE (`scripts/gen_time_golden.py`)
  and golden output digests checked on every CI platform and wasm32.
