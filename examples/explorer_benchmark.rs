//! Compare cold and reused orbit work with identical observer requests and JSON encoding.
use orbit_track::{
    data,
    explorer::{ExploreRequest, Explorer},
    observer::Observer,
};
use serde_json::json;
use std::{hint::black_box, path::Path, sync::Arc, time::Instant};
fn percentile(times: &mut [f64], fraction: f64) -> f64 {
    times.sort_by(f64::total_cmp);
    times[((times.len() - 1) as f64 * fraction).round() as usize]
}
fn main() {
    let iterations: usize = std::env::args()
        .nth(1)
        .map(|s| s.parse().expect("iteration count"))
        .unwrap_or(400);
    assert!(iterations >= 10);
    let catalog = Arc::new(data::catalog(Path::new("fixtures")).unwrap());
    let mut cases = Vec::new();
    for (minutes, step) in [(90, 30), (360, 30), (360, 10)] {
        let warm = Explorer::new(Arc::clone(&catalog));
        let mut request = ExploreRequest {
            catalog_id: Some(25544),
            tle: None,
            start: "epoch".into(),
            minutes,
            step_seconds: step,
            observer: Observer {
                latitude_deg: 43.4643,
                longitude_deg: -80.5204,
                altitude_km: 0.,
                min_elevation_deg: 10.,
            },
        };
        warm.explore(request.clone()).unwrap();
        let mut cold_times = Vec::new();
        let mut reuse_times = Vec::new();
        for i in 0..iterations + 20 {
            request.observer.latitude_deg = -60. + (i % 120) as f64;
            let cold = Explorer::new(Arc::clone(&catalog));
            // Alternate order to reduce systematic warm-up/order bias.
            let order = if i % 2 == 0 {
                [(&cold, false), (&warm, true)]
            } else {
                [(&warm, true), (&cold, false)]
            };
            let mut outputs = Vec::new();
            for (engine, reuse) in order {
                let start = Instant::now();
                let response = engine.explore(request.clone()).unwrap();
                black_box(serde_json::to_vec(&response).unwrap());
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(
                    response["performance"]["cache"],
                    if reuse { "hit" } else { "miss" }
                );
                if i >= 20 {
                    if reuse {
                        reuse_times.push(elapsed);
                    } else {
                        cold_times.push(elapsed);
                    }
                }
                outputs.push(response);
            }
            for field in ["track", "geojson", "observation", "orbit"] {
                assert_eq!(outputs[0][field], outputs[1][field]);
            }
        }
        let cold_median = percentile(&mut cold_times, 0.5);
        let warm_median = percentile(&mut reuse_times, 0.5);
        cases.push(json!({"samples":minutes*60/step+1,"minutes":minutes,"step_seconds":step,"iterations_per_variant":iterations,
            "cold_median_ms":cold_median,"reuse_median_ms":warm_median,"cold_p95_ms":percentile(&mut cold_times,0.95),"reuse_p95_ms":percentile(&mut reuse_times,0.95),
            "median_speedup":cold_median/warm_median,"median_reduction_percent":(1.-warm_median/cold_median)*100.,
            "cold_samples_ms":cold_times,"reuse_samples_ms":reuse_times}));
    }
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .unwrap();
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .lines()
        .find(|s| s.starts_with("model name"))
        .unwrap_or("unavailable")
        .to_owned();
    println!("{}",serde_json::to_string_pretty(&json!({"schema_version":1,"measured_at_utc":chrono::Utc::now(),"rustc":String::from_utf8_lossy(&rustc.stdout).trim(),"cpu":cpu,"architecture":std::env::consts::ARCH,"fixture_sha256":catalog[&25544].provenance.sha256,"warmup_pairs":20,"scope":"Warm release executable on one development host. Handler plus JSON encoding; no HTTP, TLS, cloud startup or browser. Identical initialized catalog and observer requests. Cold variant starts with an empty track cache each request; reuse variant holds one orbit. Excludes catalog loading and Explorer construction. This compares two Rust execution paths, not programming languages.","parity":"Track, GeoJSON, observation and orbit metadata compared exactly for every measured pair; timing/cache metadata excluded.","cases":cases})).unwrap());
}
