use crate::{Error, Result};
use chrono::{DateTime, Utc};
/// IAU-1982 GMST, UTC approximates UT1; no polar motion or equation of equinoxes.
pub fn gmst(at: DateTime<Utc>) -> f64 {
    let jd =
        2440587.5 + at.timestamp() as f64 / 86400.0 + at.timestamp_subsec_nanos() as f64 / 86400e9;
    let t = (jd - 2451545.0) / 36525.0;
    (280.46061837 + 360.98564736629 * (jd - 2451545.0) + 0.000387933 * t * t
        - t * t * t / 38710000.0)
        .to_radians()
        .rem_euclid(std::f64::consts::TAU)
}
pub fn rotate_teme(position: [f64; 3], angle: f64) -> [f64; 3] {
    let (s, c) = angle.sin_cos();
    [
        c * position[0] + s * position[1],
        -s * position[0] + c * position[1],
        position[2],
    ]
}
pub fn normalize_longitude(degrees: f64) -> f64 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}
/// WGS84 ellipsoid. Returns geodetic latitude/longitude in degrees, altitude in km.
pub fn geodetic([x, y, z]: [f64; 3]) -> Result<[f64; 3]> {
    const A: f64 = 6378.137;
    const F: f64 = 1.0 / 298.257223563;
    let b = A * (1.0 - F);
    let e2 = F * (2.0 - F);
    let p = x.hypot(y);
    if ![x, y, z].iter().all(|n| n.is_finite()) || p.hypot(z) < 1.0 {
        return Err(Error::new(
            "PROPAGATION_FAILED",
            "invalid Earth-fixed position",
        ));
    }
    if p < 1e-9 {
        return Ok([90.0 * z.signum(), 0.0, z.abs() - b]);
    }
    let mut lat = z.atan2(p * (1.0 - e2));
    for _ in 0..12 {
        let n = A / (1.0 - e2 * lat.sin().powi(2)).sqrt();
        let next = (z + e2 * n * lat.sin()).atan2(p);
        if (next - lat).abs() < 1e-14 {
            lat = next;
            break;
        }
        lat = next;
    }
    let n = A / (1.0 - e2 * lat.sin().powi(2)).sqrt();
    let altitude = if lat.cos().abs() > 0.1 {
        p / lat.cos() - n
    } else {
        z / lat.sin() - n * (1.0 - e2)
    };
    Ok([
        lat.to_degrees(),
        normalize_longitude(y.atan2(x).to_degrees()),
        altitude,
    ])
}
/// Linear interpolation at the dateline is for display only, not another SGP4 sample.
/// GeoJSON altitude uses meters per RFC 7946, unlike the API's explicit altitude_km.
pub fn split_antimeridian(points: &[[f64; 3]]) -> Vec<Vec<[f64; 3]>> {
    let mut out: Vec<Vec<[f64; 3]>> = Vec::new();
    for p in points {
        if out.is_empty() {
            out.push(vec![*p]);
            continue;
        }
        let previous = *out.last().unwrap().last().unwrap();
        let delta = p[0] - previous[0];
        if delta.abs() > 180.0 {
            let adjusted = p[0] - 360.0 * delta.signum();
            let edge = if previous[0] >= 0.0 { 180.0 } else { -180.0 };
            let fraction = (edge - previous[0]) / (adjusted - previous[0]);
            let latitude = previous[1] + fraction * (p[1] - previous[1]);
            let altitude = previous[2] + fraction * (p[2] - previous[2]);
            out.last_mut().unwrap().push([edge, latitude, altitude]);
            out.push(vec![[-edge, latitude, altitude]]);
        }
        out.last_mut().unwrap().push(*p);
    }
    out
}
