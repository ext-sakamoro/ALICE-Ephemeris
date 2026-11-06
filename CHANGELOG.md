# Changelog

All notable changes to this project are documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Law 識別子 8 本 (`tdb-spice-et` / `tdb-fb1990` / `julian-date` / `teme-itrs-gmst82` / `iau2006-2000a-cio` / `earth-epv00` / `moon98` / `sun-moon-horizon`) は `alice-det-math` 全体の識別子でなく、この crate が呼ぶ 8 関数の識別子 (`law::DET_MATH_FUNCTIONS` / `DET_MATH_PINS`) を畳む 基準の格子の出力の digest は全部不変 前の値は `law::PREVIOUS_SEMANTICS_IDS` に残し、`law::names_law` が両方を読む 呼ばない関数が動いても識別子は動かず、呼ぶ関数が動けば compile が止まる `alice-det-math` は rev から `branch = "main"` + version 0.6 に

### Added

- `star_table`: Bright Star Catalog の等級 6.0 までの 5080 星 (`BRIGHTEST`、`STAR_COUNT`) と `stars`: 地平座標への変換
- sky module: `moon_alt_az_lit`、`stars_alt_az_mag` 表は持たず、page が `sky/stars.bin` を `stars_table` に書いて `stars_table_written` で渡す (表が無ければ NaN)

### Changed

- sky module: Moon の級数の範囲外の日付と 2^63 以上の時刻は NaN を返す

### Fixed

- sky module: build 機に rust-src があるかで bytes が変わっていた (標準ライブラリの path の名前が違う) どちらでも `/rustc/<commit>` にそろえ、sysroot の写しの path は path scan で fail にする

### Changed

- **Breaking:** 破壊的変更: `sunmoon::moon98` は `Pv` でなく `MoonPv { geocentric, in_range }` を返す `in_range` は日付が ERFA の精度の範囲 (ERFA の注 3 の ELP/MPP02 との比較の 1950–2100、J2000.0 から T が −0.5 から +1.0 Julian 世紀) にあるか (ERFA の routine は状態を返さない) 位置と速度の値と `law::MOON98` の識別子は変わらない
- `alice-det-math` を git rev `6384856c` (0.5.2) で引く
- manifest の authors から address を外した

### Added

- CI の browser job: `scripts/sky_wasm_repro.sh` が `sky/` を深さの違う 2 つの dir で build して同じ bytes であること、`scripts/wasm_path_scan.py` が build path (home・temp・cargo home) を含まないことを確かめる (hit と 0 module は失敗、試験 `scripts/test_wasm_path_scan.py`) root を workspace (`.`, `browser`, `sky`、default-members は `.`) にした: workspace の外の package は Cargo の metadata が絶対 path から作られ、同じ source が dir ごとに違う bytes になっていた `browser/` と `sky/` の lock は root の 1 つに統合
- `horizon::sky_at_unix_ms(unix_ms, &site)`: Unix ミリ秒 (browser の時計) で表した UTC の時刻の空 TT は UTC の表記から閏秒の表を通して作り (表の期限を過ぎたら新しい閏秒は無いとみなす)、UT1 は UTC とし (|UT1 − UTC| < 0.9 s、空の回転で ≤ 14″)、極の座標は 0 UTC の始まり (1960) より前は `TimeError::BeforeUtc` 試験 `tests/horizon_unix.rs` (2026 年 3 月の分点で太陽直下点の高度 > 89° と対蹠点 < −89°、2026 年の TT − UTC = 37 s + 32.184 s を外から与えた `horizon::sky` と 1e-11 rad 以内、1960 より前は誤り) 変異 4 本 (UT1 に TT / 閏秒なし / 月と日の入替 / ミリ秒落ち) はそれぞれ red
- `sky/` (crate `alice-ephemeris-sky`、wasm32-unknown-unknown の cdylib): web page が読む module で export は `sun_alt_az(unix_ms, latitude_deg, longitude_deg)` だけ (Law の検査は `browser/` に残る) page が `Date.now()` と度で呼び、返した pointer に太陽の高度と方位 (radian) を 2 つの `f64` で書く 整数でないミリ秒・1960 より前・地球上にない場所 (有限でない、|緯度| > 90、|経度| > 180) は NaN release (profile sky: lto、opt-level z、panic abort、build path を remap し symbol を除く) で約 155 KB 試験: `sky/tests/native.rs` (`sky/expected.txt` の 24 入力 (1972〜2096、両極・日付変更線を含む) の出力の sha256 を固定、不正な入力で NaN、分点の太陽直下点で > 89°)、`scripts/run_sky_wasm.sh` が wasm を node で同じ file に対して実行し同じ sha256、export が `memory` と `sun_alt_az` だけ、分点の太陽直下点で 89.981° と対蹠点で −89.981° を確かめる (CI の browser job) `tests/law_ids_sunmoon.rs` の `unix_sky_digest` (1972〜2099 の 12 時刻 × 5 地点) は native と browser の両方で `sky_at_unix_ms` を固定する
- CI の `browser` job: `tests/law_ids*.rs` の Law 識別子の検査と bench の太陽と月の出力の digest (11 本) を
  wasm32-unknown-unknown (web page が読む target) で build し node で実行する
  (`browser/`、`scripts/run_browser_wasm.sh`) 1 本でも trap する / 0 本 / 本数が
  `#[test]` の数と違う時は red `horizon::sky` の 1 回の時間・build の大きさ・使った
  alice-det-math の `SEMANTICS_ID` を出す (報告のみ) 検査の本体は test file の `*_check` 関数で、`#[test]` はそれを呼ぶ
- 3 つの test file (`tests/law_ids*.rs`) の SHA-256 の helper (`hex` / `digest` / `law_id`) を `browser/src/law_digest.rs`
  の 1 か所にした (これまで 3 file に同じ写し) native の試験は dev-dependency の `alice-ephemeris-browser`
  から、browser の build は自分の crate から同じ code を使う 太陽と月の bench の入力の出力の digest
  (`bench_sky_digest`、2000 点) を native と browser の両方で同じ値に照合する
  ⚠️ この digest と Law 識別子は alice-det-math の世代 (今は rev 6384856c、SEMANTICS_ID 85e08846…) の値で、
  det-math の世代を揃える変更 (Physics・SDF は rev 2d01c63f) の時にその変更と一緒に動く
- `scripts/machete_metadata_check.py` (CI の unused-deps job と preflight): どの Cargo.toml にも
  cargo-machete の ignore の指定を置かない 各 manifest を TOML として読み (table / dotted key /
  inline table / quoted key のどの書き方も同じ key になる)、`package.metadata` か
  `workspace.metadata` に `cargo-machete` があれば red、読めない manifest と manifest 0 件も red
  使い道が machete に見えない依存は、見える形にする
- `bench_sky_digest` は loop の回数でなく hash した byte 数 (`Hasher::len`) を照合する
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
