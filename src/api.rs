use crate::{
    Error,
    core::ElementSet,
    explorer::{ExploreRequest, Explorer},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderValue, Method, StatusCode, header::CONTENT_TYPE},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;
pub type Catalog = Arc<BTreeMap<u64, ElementSet>>;

/// Exact browser origins; CORS controls browser access, not API authentication.
pub fn parse_origin(input: &str) -> Result<HeaderValue, String> {
    let url = reqwest::Url::parse(input).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("use an HTTP(S) origin without a path, credentials, query, or fragment".into());
    }
    HeaderValue::from_str(&url.origin().ascii_serialization()).map_err(|e| e.to_string())
}

async fn limit_predictions(
    State(slots): State<Arc<Semaphore>>,
    request: Request,
    next: Next,
) -> Response {
    let Ok(_slot) = slots.try_acquire_owned() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(axum::http::header::RETRY_AFTER, "1")],
            Json(json!({"error":{"code":"SERVER_BUSY","message":"The explorer is busy. Please try again shortly."}})),
        ).into_response();
    };
    next.run(request).await
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self.code {
            "ELEMENTS_NOT_FOUND" => StatusCode::NOT_FOUND,
            "DATA_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
            "REQUEST_BODY_TOO_LARGE" => StatusCode::PAYLOAD_TOO_LARGE,
            _ => StatusCode::BAD_REQUEST,
        };
        (status, Json(json!({"error":self}))).into_response()
    }
}
#[derive(Deserialize)]
struct TrackQuery {
    #[serde(default = "epoch")]
    start: String,
    #[serde(default = "minutes")]
    minutes: u32,
    #[serde(default = "step")]
    step_seconds: u32,
    #[serde(default)]
    teme: bool,
    #[serde(default)]
    format: Option<String>,
}
fn epoch() -> String {
    "epoch".into()
}
fn minutes() -> u32 {
    90
}
fn step() -> u32 {
    30
}
#[derive(Deserialize)]
struct PositionQuery {
    at: String,
}
type QueryResult<T> = std::result::Result<Query<T>, axum::extract::rejection::QueryRejection>;
async fn track(
    State(c): State<Catalog>,
    Path(id): Path<u64>,
    q: QueryResult<TrackQuery>,
) -> Result<Json<Value>, Error> {
    let Query(q) = q.map_err(|e| Error::new("INVALID_QUERY", e))?;
    if q.format
        .as_ref()
        .is_some_and(|f| f != "json" && f != "geojson" && f != "explorer")
    {
        return Err(Error::new(
            "INVALID_QUERY",
            "format must be json, geojson, or explorer",
        ));
    }
    tokio::task::spawn_blocking(move || {
        let set = c.get(&id).ok_or_else(|| {
            Error::new("ELEMENTS_NOT_FOUND", format!("catalog ID {id} not loaded"))
        })?;
        let track = set.track(&q.start, q.minutes, q.step_seconds, q.teme)?;
        Ok(Json(match q.format.as_deref() {
            Some("geojson") => track.geojson(),
            Some("explorer") => json!({"geojson":track.geojson(),"track":track}),
            _ => serde_json::to_value(track).map_err(|e| Error::new("PROPAGATION_FAILED", e))?,
        }))
    })
    .await
    .map_err(|e| Error::new("DATA_ERROR", e))?
}
async fn position(
    State(c): State<Catalog>,
    Path(id): Path<u64>,
    q: QueryResult<PositionQuery>,
) -> Result<Json<Value>, Error> {
    let Query(q) = q.map_err(|e| Error::new("INVALID_QUERY", e))?;
    let set = c
        .get(&id)
        .ok_or_else(|| Error::new("ELEMENTS_NOT_FOUND", "satellite not loaded"))?;
    Ok(Json(
        serde_json::to_value(set.track(&q.at, 0, 30, false)?)
            .map_err(|e| Error::new("PROPAGATION_FAILED", e))?,
    ))
}
pub fn router(catalog: Catalog) -> Router {
    router_with_origins(catalog, vec![])
}

async fn explore(
    State(engine): State<Arc<Explorer>>,
    body: Result<Json<ExploreRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, Error> {
    let Json(request) = body.map_err(|e| {
        Error::new(
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                "REQUEST_BODY_TOO_LARGE"
            } else {
                "INVALID_QUERY"
            },
            e.body_text(),
        )
    })?;
    tokio::task::spawn_blocking(move || engine.explore(request).map(Json))
        .await
        .map_err(|e| Error::new("DATA_ERROR", e))?
}

pub fn router_with_origins(catalog: Catalog, origins: Vec<HeaderValue>) -> Router {
    let interactive = Router::new()
        .route("/v1/explore", post(explore))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .with_state(Arc::new(Explorer::new(Arc::clone(&catalog))));
    let predictions = Router::new()
        .route("/v1/satellites/{id}/track", get(track))
        .route("/v1/satellites/{id}/position", get(position))
        .with_state(Arc::clone(&catalog))
        .merge(interactive)
        .route_layer(middleware::from_fn_with_state(
            Arc::new(Semaphore::new(8)),
            limit_predictions,
        ));
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../web/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../web/app.js"),
                )
            }),
        )
        .route(
            "/api-client.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../web/api-client.js"),
                )
            }),
        )
        .route(
            "/config.js",
            get(|| async {
                (
                    [
                        (
                            axum::http::header::CONTENT_TYPE,
                            "text/javascript; charset=utf-8",
                        ),
                        (axum::http::header::CACHE_CONTROL, "no-store"),
                    ],
                    include_str!("../web/config.js"),
                )
            }),
        )
        .route(
            "/styles.css",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../web/styles.css"),
                )
            }),
        )
        .route(
            "/basemap.svg",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
                    include_str!("../web/basemap.svg"),
                )
            }),
        )
        .route(
            "/healthz",
            get(|| async { Json(json!({"service":"orbit-track","status":"ok"})) }),
        )
        .route(
            "/v1/satellites",
            get(|State(c): State<Catalog>| async move {
                Json(json!({"satellites":c.values().map(|s|s.metadata()).collect::<Vec<_>>()}))
            }),
        )
        .with_state(catalog)
        .merge(predictions)
        .layer(
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers([CONTENT_TYPE])
                .max_age(std::time::Duration::from_secs(3600)),
        )
}
