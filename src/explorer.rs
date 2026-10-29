//! Reuse immutable orbit results across changes to the ground observer.
use crate::{
    Error, Result,
    api::Catalog,
    core::{ElementSet, Track},
    observer::{self, Observer},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};

const MAX_CACHED_SAMPLES: usize = 32_768;
const MAX_CACHED_TRACKS: usize = 32;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExploreRequest {
    pub catalog_id: Option<u64>,
    pub tle: Option<String>,
    pub start: String,
    pub minutes: u32,
    pub step_seconds: u32,
    pub observer: Observer,
}
#[derive(PartialEq)]
struct Key {
    sha256: String,
    start: String,
    minutes: u32,
    step: u32,
}
struct Orbit {
    track: Track,
    geojson: Value,
}
#[derive(Default)]
struct Cache {
    entries: VecDeque<(Key, Arc<Orbit>)>,
    samples: usize,
}
impl Cache {
    fn get(&mut self, key: &Key) -> Option<Arc<Orbit>> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let orbit = Arc::clone(&entry.1);
        self.entries.push_back(entry);
        Some(orbit)
    }
    fn insert(&mut self, key: Key, orbit: Arc<Orbit>) {
        if self.get(&key).is_some() {
            return;
        }
        while self.entries.len() >= MAX_CACHED_TRACKS
            || self.samples + orbit.track.samples.len() > MAX_CACHED_SAMPLES
        {
            if let Some((_, oldest)) = self.entries.pop_front() {
                self.samples -= oldest.track.samples.len();
            } else {
                break;
            }
        }
        self.samples += orbit.track.samples.len();
        self.entries.push_back((key, orbit));
    }
}
pub struct Explorer {
    pub catalog: Catalog,
    cache: Mutex<Cache>,
}
impl Explorer {
    pub fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            cache: Mutex::new(Cache::default()),
        }
    }
    pub fn explore(&self, request: ExploreRequest) -> Result<Value> {
        let total_start = Instant::now();
        request.observer.validate()?;
        let custom;
        let (set, cache_allowed) = match (request.catalog_id, request.tle.as_deref()) {
            (Some(id), None) => (
                self.catalog
                    .get(&id)
                    .ok_or_else(|| Error::new("ELEMENTS_NOT_FOUND", "satellite not loaded"))?,
                true,
            ),
            (None, Some(raw)) => {
                custom = ElementSet::parse(raw.as_bytes(), None)?;
                (&custom, false)
            }
            _ => {
                return Err(Error::new(
                    "INVALID_QUERY",
                    "provide exactly one of catalog_id or tle",
                ));
            }
        };
        let key = Key {
            sha256: set.provenance.sha256.clone(),
            start: set.time(&request.start)?.to_rfc3339(),
            minutes: request.minutes,
            step: request.step_seconds,
        };
        let orbit_start = Instant::now();
        let cached = if cache_allowed {
            self.cache
                .lock()
                .map_err(|e| Error::new("DATA_ERROR", e))?
                .get(&key)
        } else {
            None
        };
        let hit = cached.is_some();
        let orbit = match cached {
            Some(orbit) => orbit,
            None => {
                let track =
                    set.track(&request.start, request.minutes, request.step_seconds, false)?;
                let orbit = Arc::new(Orbit {
                    geojson: track.geojson(),
                    track,
                });
                if cache_allowed {
                    self.cache
                        .lock()
                        .map_err(|e| Error::new("DATA_ERROR", e))?
                        .insert(key, Arc::clone(&orbit));
                }
                orbit
            }
        };
        let orbit_ms = orbit_start.elapsed().as_secs_f64() * 1000.0;
        let observer_start = Instant::now();
        let analysis = observer::analyze(&orbit.track, request.observer)?;
        let observer_ms = observer_start.elapsed().as_secs_f64() * 1000.0;
        let mut response = json!({"track":orbit.track,"geojson":orbit.geojson,"observation":analysis,"orbit":set.orbit_summary()});
        response["performance"] = json!({
            "language":"Rust", "propagator":"sgp4 2.4.0", "http_framework":"Axum + Tokio",
            "cache": if !cache_allowed { "bypass" } else if hit { "hit" } else { "miss" },
            "propagated_samples": if hit { 0 } else { orbit.track.samples.len() },
            "returned_samples": orbit.track.samples.len(), "orbit_ms":orbit_ms, "observer_ms":observer_ms,
            "compute_ms":total_start.elapsed().as_secs_f64()*1000.0,
            "scope":"Rust handler work through response assembly; excludes final JSON encoding, request queueing and network transit",
            "cache_max_tracks":MAX_CACHED_TRACKS,"cache_max_samples":MAX_CACHED_SAMPLES
        });
        Ok(response)
    }
}
