//! WGS84 ground observers and sampled, geometric access windows.
use crate::{Error, Result, core::Track};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observer {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    #[serde(default)]
    pub altitude_km: f64,
    #[serde(default = "default_elevation")]
    pub min_elevation_deg: f64,
}
fn default_elevation() -> f64 {
    10.0
}
impl Observer {
    pub fn validate(&self) -> Result<()> {
        if !self.latitude_deg.is_finite()
            || !(-90.0..=90.0).contains(&self.latitude_deg)
            || !self.longitude_deg.is_finite()
            || !(-180.0..=180.0).contains(&self.longitude_deg)
            || !self.altitude_km.is_finite()
            || !(-0.5..=10.0).contains(&self.altitude_km)
            || !self.min_elevation_deg.is_finite()
            || !(0.0..=90.0).contains(&self.min_elevation_deg)
        {
            return Err(Error::new(
                "INVALID_OBSERVER",
                "latitude must be -90..90, longitude -180..180, altitude -0.5..10 km, elevation threshold 0..90 degrees",
            ));
        }
        Ok(())
    }
}
pub fn ecef(latitude: f64, longitude: f64, altitude: f64) -> [f64; 3] {
    let (sin_lat, cos_lat) = latitude.to_radians().sin_cos();
    let (sin_lon, cos_lon) = longitude.to_radians().sin_cos();
    let f = 1.0 / 298.257223563;
    let e2 = f * (2.0 - f);
    let n = 6378.137 / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    [
        (n + altitude) * cos_lat * cos_lon,
        (n + altitude) * cos_lat * sin_lon,
        (n * (1.0 - e2) + altitude) * sin_lat,
    ]
}
#[derive(Debug, Serialize)]
pub struct LookAngle {
    pub azimuth_deg: Option<f64>,
    pub elevation_deg: f64,
    pub range_km: f64,
    pub above_threshold: bool,
}
pub fn look_angle(observer: &Observer, satellite: [f64; 3]) -> LookAngle {
    let station = ecef(
        observer.latitude_deg,
        observer.longitude_deg,
        observer.altitude_km,
    );
    let [x, y, z] = std::array::from_fn(|i| satellite[i] - station[i]);
    let (slat, clat) = observer.latitude_deg.to_radians().sin_cos();
    let (slon, clon) = observer.longitude_deg.to_radians().sin_cos();
    let east = -slon * x + clon * y;
    let north = -slat * clon * x - slat * slon * y + clat * z;
    let up = clat * clon * x + clat * slon * y + slat * z;
    let horizontal = east.hypot(north);
    let elevation = up.atan2(horizontal).to_degrees();
    LookAngle {
        azimuth_deg: (horizontal > 1e-8).then(|| east.atan2(north).to_degrees().rem_euclid(360.0)),
        elevation_deg: elevation,
        range_km: horizontal.hypot(up),
        above_threshold: elevation >= observer.min_elevation_deg,
    }
}
#[derive(Debug, Serialize)]
pub struct Pass {
    pub first_index: usize,
    pub last_index: usize,
    pub peak_index: usize,
    pub max_elevation_deg: f64,
    pub clipped_start: bool,
    pub clipped_end: bool,
}
pub fn sampled_passes(looks: &[LookAngle]) -> Vec<Pass> {
    let mut passes = Vec::new();
    let mut index = 0;
    while index < looks.len() {
        if !looks[index].above_threshold {
            index += 1;
            continue;
        }
        let first = index;
        let mut peak = index;
        while index + 1 < looks.len() && looks[index + 1].above_threshold {
            index += 1;
            if looks[index].elevation_deg > looks[peak].elevation_deg {
                peak = index;
            }
        }
        passes.push(Pass {
            first_index: first,
            last_index: index,
            peak_index: peak,
            max_elevation_deg: looks[peak].elevation_deg,
            clipped_start: first == 0,
            clipped_end: index + 1 == looks.len(),
        });
        index += 1;
    }
    passes
}
#[derive(Serialize)]
pub struct Analysis {
    pub observer: Observer,
    pub look_angles: Vec<LookAngle>,
    pub passes: Vec<Pass>,
    pub max_elevation_deg: f64,
    pub note: &'static str,
}
pub fn analyze(track: &Track, observer: Observer) -> Result<Analysis> {
    observer.validate()?;
    let look_angles: Vec<_> = track
        .samples
        .iter()
        .map(|s| {
            look_angle(
                &observer,
                ecef(s.latitude_deg, s.longitude_deg, s.altitude_km),
            )
        })
        .collect();
    let max_elevation_deg = look_angles
        .iter()
        .map(|l| l.elevation_deg)
        .fold(-90.0, f64::max);
    let passes = sampled_passes(&look_angles);
    Ok(Analysis {
        observer,
        look_angles,
        passes,
        max_elevation_deg,
        note: "Geometric line of sight at the requested sample interval. Short passes may fall between samples. No terrain, refraction, sunlight, or visual observability model. Window boundaries are sampled, not refined rise/set events.",
    })
}
