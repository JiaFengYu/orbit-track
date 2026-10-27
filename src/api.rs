use crate::{Error, core::ElementSet};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
pub type Catalog = Arc<BTreeMap<u64, ElementSet>>;
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self.code {
            "ELEMENTS_NOT_FOUND" => StatusCode::NOT_FOUND,
            "DATA_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
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
        .is_some_and(|f| f != "json" && f != "geojson")
    {
        return Err(Error::new(
            "INVALID_QUERY",
            "format must be json or geojson",
        ));
    }
    tokio::task::spawn_blocking(move || {
        let set = c.get(&id).ok_or_else(|| {
            Error::new("ELEMENTS_NOT_FOUND", format!("catalog ID {id} not loaded"))
        })?;
        let track = set.track(&q.start, q.minutes, q.step_seconds, q.teme)?;
        Ok(Json(if q.format.as_deref() == Some("geojson") {
            track.geojson()
        } else {
            serde_json::to_value(track).map_err(|e| Error::new("PROPAGATION_FAILED", e))?
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
        .route("/v1/satellites/{id}/track", get(track))
        .route("/v1/satellites/{id}/position", get(position))
        .with_state(catalog)
}
