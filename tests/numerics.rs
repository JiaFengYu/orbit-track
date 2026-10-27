use chrono::{DateTime, Duration};
use orbit_track::{core::ElementSet, frames};
use serde::Deserialize;
#[derive(Deserialize)]
struct Cases {
    list: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    line1: String,
    line2: String,
    states: Vec<State>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum State {
    Good {
        time: f64,
        position: [f64; 3],
        velocity: [f64; 3],
    },
    Bad {
        time: f64,
        error: String,
    },
}
#[test]
fn all_published_reference_vectors_in_afspc_mode() {
    let cases: Cases = toml::from_str(include_str!("../fixtures/vallado-reference.toml")).unwrap();
    let mut checked = 0;
    for case in cases.list {
        let raw = format!("{}\n{}\n", case.line1, case.line2);
        let set = ElementSet::parse(raw.as_bytes(), None).unwrap();
        for state in case.states {
            match state {
                State::Good {
                    time,
                    position,
                    velocity,
                } => {
                    let p = set
                        .constants
                        .propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(time))
                        .unwrap();
                    for axis in 0..3 {
                        assert!(
                            (p.position[axis] - position[axis]).abs() < 1e-6,
                            "position at {time}: {:?} vs {position:?}",
                            p.position
                        );
                        assert!(
                            (p.velocity[axis] - velocity[axis]).abs() < 1e-9,
                            "velocity at {time}"
                        );
                    }
                }
                State::Bad { time, error } => {
                    let e = set
                        .constants
                        .propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(time))
                        .unwrap_err();
                    assert!(matches!(
                        (e, error.as_str()),
                        (
                            sgp4::Error::NegativeSemiLatusRectum { .. },
                            "negative semi-latus rectum"
                        ) | (
                            sgp4::Error::OutOfRangePerturbedEccentricity { .. },
                            "diverging perturbed eccentricity"
                        )
                    ));
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 668);
}
#[test]
fn timestamp_wrapper_matches_vanguard_reference() {
    let set = ElementSet::parse(include_bytes!("../fixtures/vanguard.tle"), None).unwrap();
    let sample = set
        .sample(set.epoch() + Duration::minutes(360), true)
        .unwrap();
    let expected = [-7154.03120202, -3783.17682504, -3536.19412294];
    for (actual, expected) in sample.teme.unwrap().position_km.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 1e-6);
    }
}
#[test]
fn independent_frame_cases() {
    let at = DateTime::parse_from_rfc3339("2000-01-01T12:00:00Z")
        .unwrap()
        .to_utc();
    assert!((frames::gmst(at).to_degrees() - 280.46061837).abs() < 1e-7);
    let rotated = frames::rotate_teme([1., 0., 0.], std::f64::consts::FRAC_PI_2);
    assert!(rotated[0].abs() < 1e-14);
    assert!((rotated[1] + 1.).abs() < 1e-14);
    let equator = frames::geodetic([6778.137, 0., 0.]).unwrap();
    assert!(equator[0].abs() < 1e-10);
    assert!((equator[2] - 400.).abs() < 1e-9);
    let pole = frames::geodetic([0., 0., 6356.752314245 + 100.]).unwrap();
    assert_eq!(pole[0], 90.);
    assert!((pole[2] - 100.).abs() < 1e-6);
    // Independent forward ellipsoid formula at 45N, 30E, altitude 400 km.
    let lat = 45f64.to_radians();
    let lon = 30f64.to_radians();
    let e2 = 0.0066943799901413165;
    let n = 6378.137 / (1. - e2 * lat.sin().powi(2)).sqrt();
    let p = [
        (n + 400.) * lat.cos() * lon.cos(),
        (n + 400.) * lat.cos() * lon.sin(),
        (n * (1. - e2) + 400.) * lat.sin(),
    ];
    let result = frames::geodetic(p).unwrap();
    assert!((result[0] - 45.).abs() < 1e-9);
    assert!((result[1] - 30.).abs() < 1e-9);
    assert!((result[2] - 400.).abs() < 1e-6);
    assert!(frames::geodetic([0., 0., 0.]).is_err());
    assert!(frames::geodetic([f64::NAN, 0., 0.]).is_err());
    assert_eq!(frames::normalize_longitude(180.), -180.);
}
#[test]
fn antimeridian_splits_in_both_directions_and_at_high_latitude() {
    for points in [
        [[170., 80., 400000.], [-170., 82., 410000.]],
        [[-170., -80., 400000.], [170., -82., 410000.]],
    ] {
        let split = frames::split_antimeridian(&points);
        assert_eq!(split.len(), 2);
        assert!((split[0][1][0].abs() - 180.).abs() < 1e-10);
        assert_eq!(split[0][1][0], -split[1][0][0]);
        assert_eq!(split[0][1][2], 405000.);
        for line in split {
            for pair in line.windows(2) {
                assert!((pair[1][0] - pair[0][0]).abs() <= 180.);
            }
        }
    }
}
#[test]
fn input_time_limits_and_freshness_are_explicit() {
    let raw = include_bytes!("../fixtures/iss.tle");
    let set = ElementSet::parse(raw, None).unwrap();
    let mut corrupt = raw.to_vec();
    corrupt[80] = b'3';
    assert!(ElementSet::parse(&corrupt, None).is_err());
    assert!(ElementSet::parse(b"garbage", None).is_err());
    let mut lines = std::str::from_utf8(raw)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines[2].replace_range(2..7, "25545");
    let checksum: u32 = lines[2].as_bytes()[..68]
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
    lines[2].replace_range(68..69, &(checksum % 10).to_string());
    assert!(
        ElementSet::parse(lines.join("\n").as_bytes(), None)
            .err()
            .unwrap()
            .message
            .contains("catalog")
    );
    assert!(set.time("2020-07-12T12:00:00").is_err());
    assert_eq!(
        set.time("2020-07-12T14:00:00+02:00").unwrap(),
        set.time("2020-07-12T12:00:00Z").unwrap()
    );
    assert_eq!(
        set.track("epoch", 90, 30, false).unwrap().samples.len(),
        181
    );
    assert_eq!(
        set.track("epoch", 360, 10, false).unwrap().samples.len(),
        2161
    );
    for (minutes, step) in [(361, 30), (90, 0), (90, 9), (90, 31)] {
        assert!(set.track("epoch", minutes, step, false).is_err());
    }
    let stale = set
        .track(
            &(set.epoch() + Duration::days(4)).to_rfc3339(),
            90,
            30,
            false,
        )
        .unwrap();
    assert!(stale.warnings.iter().any(|v| v["code"] == "STALE_ELEMENTS"));
    assert!(set.sample(set.epoch() + Duration::days(15), false).is_err());
    assert!(set.sample(set.epoch() - Duration::days(1), false).is_ok());
}
#[test]
fn predictions_are_reproducible_and_geojson_uses_meters() {
    let set = ElementSet::parse(include_bytes!("../fixtures/iss.tle"), None).unwrap();
    let a = set.track("epoch", 90, 30, false).unwrap();
    let b = set.track("epoch", 90, 30, false).unwrap();
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    let geo = a.geojson();
    assert_eq!(geo["geometry"]["type"], "MultiLineString");
    assert_eq!(
        geo["geometry"]["coordinates"][0][0][2].as_f64().unwrap(),
        a.samples[0].altitude_km * 1000.
    );
}
