use chrono::{Duration, Utc};
use orbit_track::{core::ElementSet, data};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
};
fn provider(body: &'static [u8], status: &'static str) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let task = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut input = [0; 8192];
        let _ = stream.read(&mut input).unwrap();
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    });
    (format!("http://{address}/elements"), task)
}
#[test]
fn cache_reuse_failure_policy_and_immutable_provenance() {
    let d = tempfile::tempdir().unwrap();
    let cache = d.path().join("cache");
    let output = d.path().join("iss.tle");
    let now = Utc::now();
    let (url, server) = provider(include_bytes!("../fixtures/iss.tle"), "200 OK");
    let first = data::fetch(&url, 25544, &cache, &output, false, now).unwrap();
    server.join().unwrap();
    assert!(!first.from_cache);
    let original = std::fs::read(&output).unwrap();
    let second = data::fetch(
        &url,
        25544,
        &cache,
        &output,
        false,
        now + Duration::hours(1),
    )
    .unwrap();
    assert!(second.from_cache);
    assert_eq!(
        first.elements.fetched_at_utc,
        second.elements.fetched_at_utc
    );
    assert!(
        data::fetch(
            &url,
            25544,
            &cache,
            &output,
            false,
            now + Duration::hours(7)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), original);
    let fallback =
        data::fetch(&url, 25544, &cache, &output, true, now + Duration::hours(7)).unwrap();
    assert!(fallback.warning.is_some());
    let set = data::load(&output).unwrap();
    assert!(
        set.track("epoch", 90, 30, false)
            .unwrap()
            .warnings
            .iter()
            .any(|w| w["code"] == "CACHED_AFTER_FETCH_FAILURE")
    );
    let immutable = data::load(&cache.join(first.elements.sha256).join("elements.tle")).unwrap();
    assert!(immutable.provenance.cache_warning.is_none());
}
#[test]
fn provider_errors_never_replace_output() {
    for (body, status) in [
        (b"not TLE".as_slice(), "200 OK"),
        (
            include_bytes!("../fixtures/vanguard.tle").as_slice(),
            "200 OK",
        ),
        (b"try later".as_slice(), "429 Too Many Requests"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("iss.tle");
        std::fs::write(&out, "preserve").unwrap();
        let (url, server) = provider(body, status);
        assert!(
            data::fetch(
                &url,
                25544,
                &d.path().join("cache"),
                &out,
                false,
                Utc::now()
            )
            .is_err()
        );
        server.join().unwrap();
        assert_eq!(std::fs::read_to_string(out).unwrap(), "preserve");
    }
}
#[test]
fn provenance_tampering_is_rejected() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("iss.tle");
    std::fs::write(&path, include_bytes!("../fixtures/iss.tle")).unwrap();
    let set = data::load(&path).unwrap();
    let mut p = set.provenance;
    p.sha256 = "wrong".into();
    std::fs::write(
        path.with_extension("meta.json"),
        serde_json::to_vec(&p).unwrap(),
    )
    .unwrap();
    assert!(data::load(&path).is_err());
}
#[tokio::test]
async fn endpoints_and_input_errors() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::Value;
    use tower::ServiceExt;
    let set = ElementSet::parse(include_bytes!("../fixtures/iss.tle"), None).unwrap();
    let app = orbit_track::api::router(Arc::new(BTreeMap::from([(25544, set)])));
    for (path, status, code) in [
        ("/healthz", 200, None),
        ("/v1/satellites", 200, None),
        ("/v1/satellites/25544/track?start=epoch", 200, None),
        ("/v1/satellites/25544/position?at=epoch", 200, None),
        ("/v1/satellites/999/track", 404, Some("ELEMENTS_NOT_FOUND")),
        (
            "/v1/satellites/25544/track?minutes=361",
            400,
            Some("REQUEST_TOO_LARGE"),
        ),
        (
            "/v1/satellites/25544/track?start=invalid",
            400,
            Some("INVALID_TIME"),
        ),
        (
            "/v1/satellites/25544/track?minutes=-1",
            400,
            Some("INVALID_QUERY"),
        ),
        (
            "/v1/satellites/25544/track?format=bad",
            400,
            Some("INVALID_QUERY"),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 2_000_000).await.unwrap())
                .unwrap();
        if let Some(code) = code {
            assert_eq!(value["error"]["code"], code);
        } else if path.contains("track") {
            assert_eq!(value["samples"].as_array().unwrap().len(), 181);
            assert_eq!(value["model"]["gravity_model"], "WGS72");
        }
    }
}
