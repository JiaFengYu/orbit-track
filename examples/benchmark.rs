use orbit_track::{core::ElementSet, frames};
use std::{hint::black_box, time::Instant};
fn measure(mut f: impl FnMut(), iterations: u32) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed().as_secs_f64() * 1000.0 / iterations as f64
}
fn main() {
    let set = ElementSet::parse(include_bytes!("../fixtures/iss.tle"), None).unwrap();
    let track = set.track("epoch", 90, 30, false).unwrap();
    let propagation_ms = measure(
        || {
            for i in 0..181 {
                black_box(
                    set.constants
                        .propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(i as f64 / 2.0))
                        .unwrap(),
                );
            }
        },
        1000,
    );
    let states: Vec<_> = (0..181)
        .map(|i| {
            set.constants
                .propagate_afspc_compatibility_mode(sgp4::MinutesSinceEpoch(i as f64 / 2.0))
                .unwrap()
                .position
        })
        .collect();
    let conversion_ms = measure(
        || {
            for (i, state) in states.iter().enumerate() {
                let at = set.epoch() + chrono::Duration::seconds(i as i64 * 30);
                black_box(frames::geodetic(frames::rotate_teme(*state, frames::gmst(at))).unwrap());
            }
        },
        1000,
    );
    let full_track_ms = measure(
        || {
            black_box(set.track("epoch", 90, 30, false).unwrap());
        },
        1000,
    );
    let serialization_ms = measure(
        || {
            black_box(serde_json::to_vec(&track).unwrap());
        },
        1000,
    );
    println!(
        "{}",
        serde_json::json!({"iterations":1000,"samples":181,"fixture_sha256":set.provenance.sha256,"propagation_ms":propagation_ms,"conversion_ms":conversion_ms,"full_track_ms":full_track_ms,"serialization_ms":serialization_ms,"scope":"warm release build; no network; mean milliseconds per 181-sample track"})
    );
}
