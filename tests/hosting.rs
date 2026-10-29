use axum::{
    body::{Body, to_bytes},
    http::{Request, header},
};
use orbit_track::{api, core::ElementSet};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use tower::ServiceExt;

fn app() -> axum::Router {
    let set = ElementSet::parse(include_bytes!("../fixtures/iss.tle"), None).unwrap();
    api::router_with_origins(
        Arc::new(BTreeMap::from([(25544, set)])),
        vec![api::parse_origin("https://jiafengyu.github.io").unwrap()],
    )
}

async fn json(path: &str) -> Value {
    let response = app()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    serde_json::from_slice(&to_bytes(response.into_body(), 2_000_000).await.unwrap()).unwrap()
}

#[tokio::test]
async fn explorer_response_matches_existing_numerical_and_geojson_contracts() {
    let path = "/v1/satellites/25544/track?start=epoch&minutes=90&step_seconds=30";
    let combined = json(&format!("{path}&format=explorer")).await;
    assert_eq!(combined["track"], json(path).await);
    assert_eq!(
        combined["geojson"],
        json(&format!("{path}&format=geojson")).await
    );
    assert_eq!(combined["track"]["samples"].as_array().unwrap().len(), 181);
    let point = json("/v1/satellites/25544/track?minutes=0&format=explorer").await;
    assert_eq!(point["geojson"]["geometry"]["type"], "Point");
}

#[tokio::test]
async fn only_configured_browser_origins_receive_cors_headers_including_errors() {
    for path in ["/v1/satellites", "/v1/satellites/999/track"] {
        for origin in [
            "https://jiafengyu.github.io",
            "https://unrelated.example",
            "null",
        ] {
            let response = app()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header(header::ORIGIN, origin)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let allow = response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
            if origin == "https://jiafengyu.github.io" {
                assert_eq!(allow.unwrap(), origin);
            } else {
                assert!(allow.is_none());
            }
            assert!(
                response
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                    .is_none()
            );
            assert!(
                response
                    .headers()
                    .get(header::VARY)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .contains("origin")
            );
        }
    }
    let preflight = app()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/v1/satellites")
                .header(header::ORIGIN, "https://jiafengyu.github.io")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        preflight.headers()[header::ACCESS_CONTROL_ALLOW_METHODS],
        "GET,POST"
    );
    assert_eq!(
        preflight.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://jiafengyu.github.io"
    );
}

#[test]
fn deployment_origins_and_public_binding_must_be_explicit() {
    for origin in [
        "*",
        "null",
        "https://example.com/path",
        "https://user:pass@example.com",
        "https://example.com?token=secret",
        "file:///tmp/page",
    ] {
        assert!(api::parse_origin(origin).is_err(), "{origin}");
    }
    assert_eq!(
        api::parse_origin("https://jiafengyu.github.io/").unwrap(),
        "https://jiafengyu.github.io"
    );
    assert_eq!(
        api::parse_origin("http://127.0.0.1:8792").unwrap(),
        "http://127.0.0.1:8792"
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_orbit-track"))
        .args(["serve", "--listen", "0.0.0.0:0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--allow-public")
    );
}
