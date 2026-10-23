//! The Sun and the Moon in an observer's sky: topocentric altitude and
//! azimuth, and the Moon's phase.
//!
//! The geocentric directions come from [`crate::sunmoon`] (the Sun as the
//! negated heliocentric Earth of `eraEpv00`, the Moon from `eraMoon98`). They
//! are rotated into the ITRS by the IAU 2006/2000A celestial-to-terrestrial
//! matrix ([`crate::frames::c2t06a`], with the caller's UT1 and pole
//! coordinates), the site's geocentric position on the WGS84 ellipsoid (ERFA
//! `eraGd2gce`) is subtracted, so the Moon's parallax (up to 1°) is included,
//! and the result is resolved along the site's east, north and up.
//!
//! The directions are geometric: no light time, no aberration (≤ 21″ for
//! either body) and no refraction. TT stands in for TDB in the Earth model
//! (|TDB − TT| ≤ 1.7 ms moves the Earth by ≤ 51 m).

use alice_det_math::{atan2_64, sin_cos64, sqrt64};

use crate::frames::{c2t06a, mxv, pfw06, rx, rz, Mat3, IDENTITY};
use crate::jd::Jd2;
use crate::sunmoon::{epv00, moon98};

/// The astronomical unit, m (IAU 2012).
const DAU: f64 = 149_597_870_700.0;
/// WGS84 equatorial radius, m.
pub const WGS84_A: f64 = 6_378_137.0;
/// WGS84 flattening.
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;

/// A site on the WGS84 ellipsoid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// Geodetic latitude, radians (north positive).
    pub latitude: f64,
    /// Longitude, radians (east positive).
    pub longitude: f64,
    /// Height above the ellipsoid, metres.
    pub height: f64,
}

impl Site {
    /// The site's geocentric position in the ITRS, metres (ERFA `eraGd2gce`
    /// with the WGS84 ellipsoid).
    pub fn itrs(&self) -> [f64; 3] {
        let (sp, cp) = sin_cos64(self.latitude);
        let w = 1.0 - WGS84_F;
        let w = w * w;
        let d = cp * cp + w * sp * sp;
        let ac = WGS84_A / sqrt64(d);
        let as_ = w * ac;
        let r = (ac + self.height) * cp;
        let (sl, cl) = sin_cos64(self.longitude);
        [r * cl, r * sl, (as_ + self.height) * sp]
    }
}

/// A direction in the site's horizon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Horizontal {
    /// Altitude above the horizon plane (normal to the ellipsoid), radians in
    /// `[−π/2, π/2]`.
    pub altitude: f64,
    /// Azimuth from north through east, radians in `[0, 2π)`.
    pub azimuth: f64,
    /// Distance from the site, au.
    pub distance: f64,
}

/// The Moon's phase as seen from the Earth's centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoonPhase {
    /// The angle Sun–Moon–Earth, radians in `[0, π]` (0 at full Moon).
    pub phase_angle: f64,
    /// The illuminated fraction of the disc, `(1 + cos i) / 2`.
    pub illuminated_fraction: f64,
    /// The Moon is east of the Sun along the ecliptic of date (between new
    /// and full Moon).
    pub waxing: bool,
}

/// The Sun and the Moon in a site's sky.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// The Sun.
    pub sun: Horizontal,
    /// The Moon.
    pub moon: Horizontal,
    /// The Moon's phase.
    pub moon_phase: MoonPhase,
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: [f64; 3]) -> f64 {
    sqrt64(dot(a, a))
}

/// The rotation from the mean ecliptic and equinox of date to the GCRS (the
/// one `eraMoon98` applies).
fn ecliptic_to_gcrs(tt: Jd2) -> Mat3 {
    let fw = pfw06(tt);
    let mut rm = IDENTITY;
    rz(fw.psib, &mut rm);
    rx(-fw.phib, &mut rm);
    rz(-fw.gamb, &mut rm);
    rm
}

fn phase(sun: [f64; 3], moon: [f64; 3], tt: Jd2) -> MoonPhase {
    let to_sun = [sun[0] - moon[0], sun[1] - moon[1], sun[2] - moon[2]];
    let to_earth = [-moon[0], -moon[1], -moon[2]];
    let i = atan2_64(norm(cross(to_sun, to_earth)), dot(to_sun, to_earth));
    let (_, ci) = sin_cos64(i);
    let pole = mxv(&ecliptic_to_gcrs(tt), [0.0, 0.0, 1.0]);
    MoonPhase {
        phase_angle: i,
        illuminated_fraction: (1.0 + ci) / 2.0,
        waxing: dot(cross(sun, moon), pole) > 0.0,
    }
}

/// The Moon's phase at TT Julian date `tt`, from the geocentric directions of
/// the Sun and the Moon.
pub fn moon_phase(tt: Jd2) -> MoonPhase {
    phase(epv00(tt).sun_geocentric()[0], moon98(tt)[0], tt)
}

/// A geocentric GCRS position (au) as seen from the site.
fn horizontal(gcrs: [f64; 3], c2t: &Mat3, site: &Site, site_itrs: [f64; 3]) -> Horizontal {
    let b = mxv(c2t, gcrs);
    let d = [
        b[0] - site_itrs[0] / DAU,
        b[1] - site_itrs[1] / DAU,
        b[2] - site_itrs[2] / DAU,
    ];
    let (sp, cp) = sin_cos64(site.latitude);
    let (sl, cl) = sin_cos64(site.longitude);
    let east = -sl * d[0] + cl * d[1];
    let north = -sp * cl * d[0] - sp * sl * d[1] + cp * d[2];
    let up = cp * cl * d[0] + cp * sl * d[1] + sp * d[2];
    let a = atan2_64(east, north);
    Horizontal {
        altitude: atan2_64(up, sqrt64(east * east + north * north)),
        azimuth: if a < 0.0 {
            a + core::f64::consts::TAU
        } else {
            a
        },
        distance: norm(d),
    }
}

/// The Sun and the Moon in `site`'s sky at TT Julian date `tt` and UT1 Julian
/// date `ut1`, with the pole coordinates `xp`, `yp` (radians; zero when
/// unknown, which moves the directions by ≤ 0.5″).
pub fn sky(tt: Jd2, ut1: Jd2, xp: f64, yp: f64, site: &Site) -> Sky {
    let c2t = c2t06a(tt, ut1, xp, yp);
    let site_itrs = site.itrs();
    let sun = epv00(tt).sun_geocentric()[0];
    let moon = moon98(tt)[0];
    Sky {
        sun: horizontal(sun, &c2t, site, site_itrs),
        moon: horizontal(moon, &c2t, site, site_itrs),
        moon_phase: phase(sun, moon, tt),
    }
}
