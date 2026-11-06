//! The stars in a site's sky: catalogue directions turned to the site's horizon.
//!
//! A star is a J2000 direction, a proper motion and a magnitude. Its direction at a time is the
//! catalogue's moved linearly by the proper motion, taken as a GCRS direction at infinity, and turned
//! by the same celestial-to-terrestrial rotation as the Sun and the Moon. Stellar aberration (at most
//! 20.5″), light deflection, parallax and refraction are left out: this draws a sky, it does not point
//! a telescope.
//!
//! Author: Moroya Sakamoto

use alice_det_math::sin_cos64;

use crate::frames::c2t06a;
use crate::horizon::{horizontal, Horizontal, Site};
use crate::jd::Jd2;

/// One star of a catalogue
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    /// Right ascension, J2000, radians
    pub ra: f64,
    /// Declination, J2000, radians
    pub dec: f64,
    /// Proper motion in right ascension (dα/dt · cos δ), radians per Julian year
    pub pm_ra: f64,
    /// Proper motion in declination, radians per Julian year
    pub pm_dec: f64,
    /// Visual magnitude
    pub mag: f32,
}

/// J2000.0 as a Julian date, and days in a Julian year
const J2000: f64 = 2_451_545.0;
const JULIAN_YEAR: f64 = 365.25;
/// Far enough that the site's offset from the Earth's centre does nothing (au)
const AT_INFINITY: f64 = 1e12;

/// The star's geocentric direction at TT Julian date `tt`, as a position at infinity (au)
fn direction(star: &Star, tt: Jd2) -> [f64; 3] {
    let years = (tt.jd1 - J2000 + tt.jd2) / JULIAN_YEAR;
    let (_, cd0) = sin_cos64(star.dec);
    let ra = if cd0.abs() > 1e-9 {
        star.ra + star.pm_ra * years / cd0
    } else {
        star.ra
    };
    let (sa, ca) = sin_cos64(ra);
    let (sd, cd) = sin_cos64(star.dec + star.pm_dec * years);
    [
        AT_INFINITY * cd * ca,
        AT_INFINITY * cd * sa,
        AT_INFINITY * sd,
    ]
}

/// Each star of `stars` in `site`'s sky at TT Julian date `tt` and UT1 Julian date `ut1`, with the pole
/// coordinates `xp`, `yp` (radians; zero when unknown), in the catalogue's order. All of them: which
/// are above the horizon is the altitude's sign
pub fn stars_in_sky<'a>(
    stars: &'a [Star],
    tt: Jd2,
    ut1: Jd2,
    xp: f64,
    yp: f64,
    site: &'a Site,
) -> impl Iterator<Item = Horizontal> + 'a {
    let c2t = c2t06a(tt, ut1, xp, yp);
    let site_itrs = site.itrs();
    stars
        .iter()
        .map(move |s| horizontal(direction(s, tt), &c2t, site, site_itrs))
}
