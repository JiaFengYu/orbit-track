use crate::{
    Error, Result,
    core::{ElementSet, Provenance, digest},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
fn io(e: impl std::fmt::Display) -> Error {
    Error::new("DATA_ERROR", e)
}
pub fn load(path: &Path) -> Result<ElementSet> {
    let mut raw = Vec::new();
    fs::File::open(path)
        .map_err(io)?
        .take(16385)
        .read_to_end(&mut raw)
        .map_err(io)?;
    let metadata = path.with_extension("meta.json");
    let provenance = if metadata.exists() {
        Some(serde_json::from_slice::<Provenance>(&fs::read(metadata).map_err(io)?).map_err(io)?)
    } else {
        None
    };
    let mut set = ElementSet::parse(&raw, provenance)?;
    if set.provenance.provider == "local-file" {
        set.provenance.source_reference = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
    }
    Ok(set)
}
pub fn catalog(dir: &Path) -> Result<BTreeMap<u64, ElementSet>> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(io)?
        .map(|p| p.map(|e| e.path()))
        .collect::<std::io::Result<_>>()
        .map_err(io)?;
    paths.sort();
    let mut result = BTreeMap::new();
    for path in paths
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "tle"))
    {
        let set = load(&path)?;
        let id = set.elements.norad_id;
        if result.insert(id, set).is_some() {
            return Err(Error::new(
                "INVALID_ELEMENTS",
                format!("duplicate catalog ID {id}; keep one active TLE per satellite"),
            ));
        }
    }
    if result.is_empty() {
        return Err(Error::new(
            "ELEMENTS_NOT_FOUND",
            "no .tle files in data directory",
        ));
    }
    Ok(result)
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(io)?;
    let result = (|| {
        f.write_all(bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
        fs::rename(&tmp, path).map_err(io)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
#[derive(Serialize, Deserialize)]
struct Index {
    sha256: String,
    checked_at: DateTime<Utc>,
    url: String,
    catalog_id: u64,
}
struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[derive(Serialize)]
pub struct FetchReport {
    pub from_cache: bool,
    pub warning: Option<String>,
    pub elements: Provenance,
    pub output: PathBuf,
}
/// Production CLI supplies the fixed CelesTrak URL. An injectable URL supports local mock-provider tests.
pub fn fetch(
    url: &str,
    id: u64,
    cache: &Path,
    output: &Path,
    allow_stale: bool,
    now: DateTime<Utc>,
) -> Result<FetchReport> {
    fs::create_dir_all(cache).map_err(io)?;
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(io)?;
    }
    let lock_path = cache.join("fetch.lock");
    let lock=fs::OpenOptions::new().write(true).create_new(true).open(&lock_path).map_err(|e|Error::new("CACHE_BUSY",format!("{e}; another fetch may be active (remove fetch.lock only after confirming no fetch is running)")))?;
    let _guard = Lock(lock_path);
    drop(lock);
    let index_path = cache.join(format!("{id}.json"));
    let old: Option<Index> = if index_path.exists() {
        Some(serde_json::from_slice(&fs::read(&index_path).map_err(io)?).map_err(io)?)
    } else {
        None
    };
    let cached = if let Some(index) = &old {
        if index.url != url || index.catalog_id != id {
            return Err(Error::new(
                "INVALID_ELEMENTS",
                "cache source/catalog mismatch",
            ));
        }
        let path = cache.join(&index.sha256).join("elements.tle");
        let set = load(&path)?;
        if set.provenance.sha256 != index.sha256 || set.elements.norad_id != id {
            return Err(Error::new(
                "INVALID_ELEMENTS",
                "cached content/catalog mismatch",
            ));
        }
        Some((fs::read(path).map_err(io)?, set.provenance))
    } else {
        None
    };
    let fresh = old.as_ref().is_some_and(|i| {
        now >= i.checked_at && now.signed_duration_since(i.checked_at).num_seconds() < 21600
    });
    let mut warning = None;
    let (raw, mut provenance, from_cache) = if fresh {
        let (raw, p) = cached.unwrap();
        (raw, p, true)
    } else {
        let request = (|| -> Result<Vec<u8>> {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent("orbit-track/0.1 (cached educational orbit explorer)")
                .build()
                .map_err(|e| Error::new("FETCH_FAILED", e))?;
            let response = client
                .get(url)
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .map_err(|e| Error::new("FETCH_FAILED", e))?;
            let mut raw = Vec::new();
            response.take(16385).read_to_end(&mut raw).map_err(io)?;
            let set = ElementSet::parse(&raw, None)?;
            if set.elements.norad_id != id {
                return Err(Error::new(
                    "INVALID_ELEMENTS",
                    "provider returned the wrong satellite",
                ));
            }
            Ok(raw)
        })();
        match request {
            Ok(raw) => {
                let hash = digest(&raw);
                let dir = cache.join(&hash);
                let p = Provenance {
                    sha256: hash.clone(),
                    provider: "CelesTrak".into(),
                    source_reference: url.into(),
                    fetched_at_utc: Some(now),
                    cache_warning: None,
                };
                let p = if dir.exists() {
                    let existing = load(&dir.join("elements.tle"))?;
                    existing.provenance
                } else {
                    // Publish an entire immutable entry with one directory rename.
                    let staging = cache.join(format!(".entry-{}", std::process::id()));
                    fs::create_dir(&staging).map_err(io)?;
                    let write = (|| {
                        atomic_write(&staging.join("elements.tle"), &raw)?;
                        atomic_write(
                            &staging.join("elements.meta.json"),
                            &serde_json::to_vec_pretty(&p).map_err(io)?,
                        )?;
                        fs::rename(&staging, &dir).map_err(io)
                    })();
                    if write.is_err() {
                        let _ = fs::remove_dir_all(&staging);
                    }
                    write?;
                    p
                };
                atomic_write(
                    &index_path,
                    &serde_json::to_vec(&Index {
                        sha256: hash,
                        checked_at: now,
                        url: url.into(),
                        catalog_id: id,
                    })
                    .map_err(io)?,
                )?;
                (raw, p, false)
            }
            Err(e) if allow_stale && cached.is_some() => {
                warning = Some(format!(
                    "Refresh failed: {e}. Using explicitly permitted cached elements."
                ));
                let (raw, p) = cached.unwrap();
                (raw, p, true)
            }
            Err(e) => return Err(e),
        }
    };
    provenance.cache_warning = warning.clone();
    // Readers reject digest mismatches during the short two-file replacement window.
    atomic_write(
        &output.with_extension("meta.json"),
        &serde_json::to_vec_pretty(&provenance).map_err(io)?,
    )?;
    atomic_write(output, &raw)?;
    Ok(FetchReport {
        from_cache,
        warning,
        elements: provenance,
        output: output.into(),
    })
}
