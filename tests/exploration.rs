use orbit_track::{
    api,
    core::ElementSet,
    data,
    explorer::{ExploreRequest, Explorer},
    frames,
    observer::{self, Observer},
};
use serde_json::json;
use std::{path::Path, sync::Arc};
fn observer() -> Observer {
    Observer {
        latitude_deg: 43.4643,
        longitude_deg: -80.5204,
        altitude_km: 0.0,
        min_elevation_deg: 10.0,
    }
}
fn request() -> ExploreRequest {
    ExploreRequest {
        catalog_id: Some(25544),
        tle: None,
        start: "epoch".into(),
        minutes: 90,
        step_seconds: 30,
        observer: observer(),
    }
}
fn explorer() -> Explorer {
    Explorer::new(Arc::new(data::catalog(Path::new("fixtures")).unwrap()))
}

#[test]
fn geodetic_inverse_and_independent_observer_axes() {
    for (lat, lon, alt) in [
        (0., 0., 0.),
        (43.4, -80.5, 0.3),
        (-65., 179., 400.),
        (89.999, 72., 35786.),
    ] {
        let result = frames::geodetic(observer::ecef(lat, lon, alt)).unwrap();
        assert!((result[0] - lat).abs() < 1e-9);
        assert!((result[1] - lon).abs() < 1e-9);
        assert!((result[2] - alt).abs() < 1e-7);
    }
    let station = Observer {
        latitude_deg: 0.,
        longitude_deg: 0.,
        ..observer()
    };
    let overhead = observer::look_angle(&station, [6878.137, 0., 0.]);
    assert!((overhead.elevation_deg - 90.).abs() < 1e-10);
    assert!((overhead.range_km - 500.).abs() < 1e-10);
    assert!(overhead.azimuth_deg.is_none());
    for (point, azimuth) in [
        ([6378.137, 1000., 0.], 90.),
        ([6378.137, 0., 1000.], 0.),
        ([6378.137, -1000., 0.], 270.),
        ([6378.137, 0., -1000.], 180.),
    ] {
        let look = observer::look_angle(&station, point);
        assert!(look.elevation_deg.abs() < 1e-10);
        assert!((look.azimuth_deg.unwrap() - azimuth).abs() < 1e-10);
        assert!((look.range_km - 1000.).abs() < 1e-10);
    }
    let opposite = observer::look_angle(&station, [-6878.137, 0., 0.]);
    assert_eq!(opposite.elevation_deg, -90.);
    assert!(!opposite.above_threshold);
}

#[test]
fn pass_windows_have_sampled_peaks_and_clipped_boundaries() {
    let looks: Vec<_> = [20., 30., -1., 5., 40., 10., -5., 15.]
        .into_iter()
        .map(|e| observer::LookAngle {
            azimuth_deg: Some(0.),
            elevation_deg: e,
            range_km: 1000.,
            above_threshold: e >= 10.,
        })
        .collect();
    let passes = observer::sampled_passes(&looks);
    assert_eq!(passes.len(), 3);
    assert_eq!(
        (
            passes[0].first_index,
            passes[0].peak_index,
            passes[0].last_index
        ),
        (0, 1, 1)
    );
    assert!(passes[0].clipped_start);
    assert!(!passes[0].clipped_end);
    assert_eq!(
        (
            passes[1].first_index,
            passes[1].peak_index,
            passes[1].last_index
        ),
        (4, 4, 5)
    );
    assert_eq!(passes[1].max_elevation_deg, 40.);
    assert!(passes[2].clipped_end);
    assert!(observer::sampled_passes(&[]).is_empty());
}

#[test]
fn observer_changes_reuse_propagation_without_reusing_geometry() {
    let engine = explorer();
    let first = engine.explore(request()).unwrap();
    let mut moved = request();
    moved.observer.latitude_deg = -33.8;
    moved.observer.longitude_deg = 151.2;
    let second = engine.explore(moved).unwrap();
    assert_eq!(first["performance"]["cache"], "miss");
    assert_eq!(second["performance"]["cache"], "hit");
    assert_eq!(second["performance"]["propagated_samples"], 0);
    assert_eq!(first["track"], second["track"]);
    assert_eq!(first["geojson"], second["geojson"]);
    assert_ne!(
        first["observation"]["look_angles"],
        second["observation"]["look_angles"]
    );
    let mut longer = request();
    longer.minutes = 180;
    assert_eq!(
        engine.explore(longer).unwrap()["performance"]["cache"],
        "miss"
    );
    let mut finer = request();
    finer.step_seconds = 10;
    assert_eq!(
        engine.explore(finer).unwrap()["performance"]["cache"],
        "miss"
    );
    let mut later = request();
    later.start = first["track"]["request"]["end_utc"]
        .as_str()
        .unwrap()
        .into();
    assert_eq!(
        engine.explore(later).unwrap()["performance"]["cache"],
        "miss"
    );
    let mut other = request();
    other.catalog_id = Some(25954);
    assert_eq!(
        engine.explore(other).unwrap()["performance"]["cache"],
        "miss"
    );
}

#[test]
fn custom_tle_and_bad_inputs_do_not_poison_the_catalog_or_cache() {
    let engine = explorer();
    let mut custom = request();
    custom.catalog_id = None;
    custom.tle = Some(include_str!("../fixtures/polar.tle").into());
    let output = engine.explore(custom.clone()).unwrap();
    assert_eq!(output["track"]["satellite"]["catalog_id"], 28057);
    assert_eq!(output["performance"]["cache"], "bypass");
    assert_eq!(
        engine.explore(custom.clone()).unwrap()["performance"]["cache"],
        "bypass"
    );
    custom.catalog_id = Some(25544);
    assert_eq!(engine.explore(custom).unwrap_err().code, "INVALID_QUERY");
    for (lat, lon, angle) in [
        (91., 0., 10.),
        (0., 181., 10.),
        (0., 0., 91.),
        (f64::NAN, 0., 10.),
    ] {
        let mut bad = request();
        bad.observer.latitude_deg = lat;
        bad.observer.longitude_deg = lon;
        bad.observer.min_elevation_deg = angle;
        assert_eq!(engine.explore(bad).unwrap_err().code, "INVALID_OBSERVER");
    }
    let mut bad = request();
    bad.minutes = 361;
    assert_eq!(engine.explore(bad).unwrap_err().code, "REQUEST_TOO_LARGE");
    assert_eq!(
        engine.explore(request()).unwrap()["performance"]["cache"],
        "miss"
    );
}

#[test]
fn all_six_bundled_orbits_produce_six_hour_explorations() {
    let engine = explorer();
    assert_eq!(engine.catalog.len(), 6);
    for id in engine.catalog.keys() {
        let mut req = request();
        req.catalog_id = Some(*id);
        req.minutes = 360;
        let output = engine.explore(req).unwrap();
        assert_eq!(output["track"]["samples"].as_array().unwrap().len(), 721);
        assert_eq!(
            output["observation"]["look_angles"]
                .as_array()
                .unwrap()
                .len(),
            721
        );
        assert!(output["orbit"]["period_minutes"].as_f64().unwrap() > 0.);
    }
}

#[test]
fn cache_bounds_evict_old_tracks_and_keep_recent_hits() {
    let engine = explorer();
    let mut first = request();
    first.minutes = 360;
    first.step_seconds = 10;
    let epoch = ElementSet::parse(include_bytes!("../fixtures/iss.tle"), None)
        .unwrap()
        .epoch();
    engine.explore(first.clone()).unwrap();
    for n in 1..20 {
        let mut next = first.clone();
        next.start = (epoch + chrono::Duration::minutes(n)).to_rfc3339();
        engine.explore(next).unwrap();
    }
    assert_eq!(
        engine.explore(first).unwrap()["performance"]["cache"],
        "miss"
    );
    let mut recent = request();
    recent.minutes = 360;
    recent.step_seconds = 10;
    recent.start = (epoch + chrono::Duration::minutes(19)).to_rfc3339();
    assert_eq!(
        engine.explore(recent).unwrap()["performance"]["cache"],
        "hit"
    );
}

#[tokio::test]
async fn json_post_cors_limits_and_old_api_compatibility() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header},
    };
    use tower::ServiceExt;
    let app = api::router_with_origins(
        Arc::new(data::catalog(Path::new("fixtures")).unwrap()),
        vec![api::parse_origin("https://jiafengyu.github.io").unwrap()],
    );
    let preflight = app
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/v1/explore")
                .header(header::ORIGIN, "https://jiafengyu.github.io")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        preflight.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://jiafengyu.github.io"
    );
    assert!(
        preflight.headers()[header::ACCESS_CONTROL_ALLOW_METHODS]
            .to_str()
            .unwrap()
            .contains("POST")
    );
    assert!(
        preflight.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
            .to_str()
            .unwrap()
            .contains("content-type")
    );
    for (payload,status) in [(json!({"catalog_id":25544,"start":"epoch","minutes":90,"step_seconds":30,"observer":{"latitude_deg":43.4,"longitude_deg":-80.5}}).to_string(),200),("{broken".into(),400),("x".repeat(40000),413)] {
        let response=app.clone().oneshot(Request::builder().method("POST").uri("/v1/explore").header(header::ORIGIN,"https://jiafengyu.github.io").header(header::CONTENT_TYPE,"application/json").body(Body::from(payload)).unwrap()).await.unwrap();
        assert_eq!(response.status(),status);
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],"https://jiafengyu.github.io");
        let body:serde_json::Value=serde_json::from_slice(&to_bytes(response.into_body(),2_000_000).await.unwrap()).unwrap();
        if status==200 {assert_eq!(body["track"]["samples"].as_array().unwrap().len(),181);} else {assert!(body.get("error").is_some());}
    }
}
