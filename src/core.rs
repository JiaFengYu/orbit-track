use crate::{Error, Result, frames};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub const MAX_AGE_DAYS: i64 = 14;
pub const WARN_AGE_HOURS: f64 = 72.0;
pub fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provenance {
    pub sha256: String,
    pub provider: String,
    pub source_reference: String,
    pub fetched_at_utc: Option<DateTime<Utc>>,
    #[serde(default)]
    pub cache_warning: Option<String>,
}
pub struct ElementSet {
    pub elements: sgp4::Elements,
    pub constants: sgp4::Constants,
    pub provenance: Provenance,
}
impl ElementSet {
    pub fn parse(raw: &[u8], provenance: Option<Provenance>) -> Result<Self> {
        if raw.len() > 16384 {
            return Err(Error::new(
                "INVALID_ELEMENTS",
                "element file exceeds 16 KiB",
            ));
        }
        let text = std::str::from_utf8(raw).map_err(|e| Error::new("INVALID_ELEMENTS", e))?;
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim_end)
            .filter(|s| !s.is_empty())
            .collect();
        let (name, l1, l2) = match lines.as_slice() {
            [l1, l2] => (None, *l1, *l2),
            [name, l1, l2] => (Some(name.trim_start_matches("0 ").to_owned()), *l1, *l2),
            _ => {
                return Err(Error::new(
                    "INVALID_ELEMENTS",
                    "expected one two-line or three-line TLE",
                ));
            }
        };
        for (line, prefix) in [(l1, b'1'), (l2, b'2')] {
            let b = line.as_bytes();
            if b.len() != 69
                || !line.is_ascii()
                || b[0] != prefix
                || b[1] != b' '
                || !b[68].is_ascii_digit()
            {
                return Err(Error::new(
                    "INVALID_ELEMENTS",
                    "TLE lines must be 69 ASCII columns with line numbers",
                ));
            }
            let sum: u32 = b[..68]
                .iter()
                .map(|b| {
                    if b.is_ascii_digit() {
                        (b - b'0') as u32
                    } else if *b == b'-' {
                        1
                    } else {
                        0
                    }
                })
                .sum();
            if sum % 10 != (b[68] - b'0') as u32 {
                return Err(Error::new("INVALID_ELEMENTS", "TLE checksum mismatch"));
            }
        }
        if l1[2..7] != l2[2..7] || !l1[2..7].trim().bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::new(
                "INVALID_ELEMENTS",
                "catalog IDs must match; numeric IDs only in v1",
            ));
        }
        let elements = sgp4::Elements::from_tle(name, l1.as_bytes(), l2.as_bytes())
            .map_err(|e| Error::new("INVALID_ELEMENTS", e))?;
        if !elements.inclination.is_finite()
            || !(0.0..=180.0).contains(&elements.inclination)
            || !elements.mean_motion.is_finite()
            || elements.mean_motion <= 0.0
            || !(0.0..1.0).contains(&elements.eccentricity)
        {
            return Err(Error::new("INVALID_ELEMENTS", "invalid orbital elements"));
        }
        let constants = sgp4::Constants::from_elements_afspc_compatibility_mode(&elements)
            .map_err(|e| Error::new("INVALID_ELEMENTS", e))?;
        let sha256 = digest(raw);
        let provenance = provenance.unwrap_or(Provenance {
            sha256: sha256.clone(),
            provider: "local-file".into(),
            source_reference: "user-supplied TLE".into(),
            fetched_at_utc: None,
            cache_warning: None,
        });
        if provenance.sha256 != sha256 {
            return Err(Error::new(
                "INVALID_ELEMENTS",
                "provenance digest does not match raw input",
            ));
        }
        Ok(Self {
            elements,
            constants,
            provenance,
        })
    }
    pub fn epoch(&self) -> DateTime<Utc> {
        self.elements.datetime.and_utc()
    }
    pub fn metadata(&self) -> Value {
        json!({"catalog_id":self.elements.norad_id,"name":self.elements.object_name,"epoch_utc":self.epoch(),"provenance":self.provenance})
    }
    pub fn time(&self, input: &str) -> Result<DateTime<Utc>> {
        if input == "epoch" {
            return Ok(self.epoch());
        }
        DateTime::parse_from_rfc3339(input)
            .map(|t| t.with_timezone(&Utc))
            .map_err(|_| {
                Error::new(
                    "INVALID_TIME",
                    "use epoch or an RFC3339 time with an explicit UTC offset",
                )
            })
    }
    pub fn sample(&self, at: DateTime<Utc>, include_teme: bool) -> Result<Sample> {
        let micros = at
            .signed_duration_since(self.epoch())
            .num_microseconds()
            .ok_or_else(|| Error::new("INVALID_TIME", "time difference overflows"))?;
        if micros.unsigned_abs() > (MAX_AGE_DAYS * 86400 * 1_000_000) as u64 {
            return Err(Error::new(
                "INVALID_TIME",
                "MVP predictions are bounded to 14 days before/after the element epoch",
            ));
        }
        let prediction = self
            .constants
            .propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(micros as f64 / 60e6))
            .map_err(|e| Error::new("PROPAGATION_FAILED", format!("{at}: {e}")))?;
        if !prediction
            .position
            .iter()
            .chain(&prediction.velocity)
            .all(|n| n.is_finite())
        {
            return Err(Error::new(
                "PROPAGATION_FAILED",
                format!("{at}: nonfinite TEME state"),
            ));
        }
        let [latitude_deg, longitude_deg, altitude_km] =
            frames::geodetic(frames::rotate_teme(prediction.position, frames::gmst(at)))?;
        if altitude_km < 0.0 {
            return Err(Error::new(
                "PROPAGATION_FAILED",
                format!("{at}: propagated below the WGS84 ellipsoid"),
            ));
        }
        Ok(Sample {
            timestamp_utc: at,
            latitude_deg,
            longitude_deg,
            altitude_km,
            element_age_hours: micros as f64 / 3.6e9,
            teme: include_teme.then_some(Teme {
                position_km: prediction.position,
                velocity_km_s: prediction.velocity,
            }),
        })
    }
    pub fn track(&self, start: &str, minutes: u32, step: u32, teme: bool) -> Result<Track> {
        if minutes > 360 || !(10..=21600).contains(&step) {
            return Err(Error::new(
                "REQUEST_TOO_LARGE",
                "minutes must be 0..360 and step_seconds 10..21600",
            ));
        }
        let seconds = minutes * 60;
        if seconds > 0 && !seconds.is_multiple_of(step) {
            return Err(Error::new(
                "INVALID_QUERY",
                "step_seconds must divide the requested duration exactly",
            ));
        }
        let count = seconds / step + 1;
        if count > 2161 {
            return Err(Error::new("REQUEST_TOO_LARGE", "at most 2161 samples"));
        }
        let start = self.time(start)?;
        let end = start
            .checked_add_signed(Duration::seconds(seconds as i64))
            .ok_or_else(|| Error::new("INVALID_TIME", "end time overflows"))?;
        self.sample(end, false)?;
        let mut samples = Vec::with_capacity(count as usize);
        for i in 0..count {
            samples.push(self.sample(start + Duration::seconds((i * step) as i64), teme)?);
        }
        let max_age = samples
            .iter()
            .map(|s| s.element_age_hours.abs())
            .fold(0.0, f64::max);
        let mut warnings = vec![
            json!({"code":"APPROXIMATE_EARTH_ORIENTATION","message":"GMST rotation with UTC approximating UT1; polar motion omitted. Predictions are not live telemetry."}),
        ];
        if max_age > WARN_AGE_HOURS {
            warnings.push(json!({"code":"STALE_ELEMENTS","message":format!("Prediction is up to {max_age:.1} hours from the element epoch (warning threshold 72 hours). This is not an accuracy guarantee.")}));
        }
        if let Some(w) = &self.provenance.cache_warning {
            warnings.push(json!({"code":"CACHED_AFTER_FETCH_FAILURE","message":w}));
        }
        Ok(Track {
            schema_version: 1,
            satellite: json!({"catalog_id":self.elements.norad_id,"name":self.elements.object_name}),
            elements: json!({"epoch_utc":self.epoch(),"sha256":self.provenance.sha256,"provider":self.provenance.provider,"source_reference":self.provenance.source_reference,"fetched_at_utc":self.provenance.fetched_at_utc}),
            model: json!({"propagator":"sgp4","version":"2.4.0","gravity_model":"WGS72","initialization_mode":"AFSPC compatibility"}),
            coordinates: json!({"state_frame":"TEME","geodetic_datum":"WGS84","earth_orientation_method":"IAU-1982 GMST; UT1 approximately UTC; no polar motion"}),
            request: json!({"start_utc":start,"end_utc":end,"step_seconds":step,"end_inclusive":true}),
            warnings,
            samples,
        })
    }
}
#[derive(Serialize, Deserialize)]
pub struct Teme {
    pub position_km: [f64; 3],
    pub velocity_km_s: [f64; 3],
}
#[derive(Serialize, Deserialize)]
pub struct Sample {
    pub timestamp_utc: DateTime<Utc>,
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    pub altitude_km: f64,
    pub element_age_hours: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teme: Option<Teme>,
}
#[derive(Serialize, Deserialize)]
pub struct Track {
    pub schema_version: u8,
    pub satellite: Value,
    pub elements: Value,
    pub model: Value,
    pub coordinates: Value,
    pub request: Value,
    pub warnings: Vec<Value>,
    pub samples: Vec<Sample>,
}
impl Track {
    pub fn geojson(&self) -> Value {
        let points: Vec<[f64; 3]> = self
            .samples
            .iter()
            .map(|s| [s.longitude_deg, s.latitude_deg, s.altitude_km * 1000.0])
            .collect();
        // A single-position request is a Point, since LineStrings need two points.
        let geometry = if points.len() == 1 {
            json!({"type":"Point","coordinates":points[0]})
        } else {
            json!({"type":"MultiLineString","coordinates":frames::split_antimeridian(&points)})
        };
        json!({"type":"Feature","geometry":geometry,"properties":{"satellite":self.satellite,"elements":self.elements,"model":self.model,"coordinates":self.coordinates,"request":self.request,"warnings":self.warnings,"altitude_unit":"meters","dateline_interpolation":"linear display-only"}})
    }
}
