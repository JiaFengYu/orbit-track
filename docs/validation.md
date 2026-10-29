# MVP validation — 2026-10-26

Validated locally on Linux x86_64, AMD EPYC 9654 96-Core Processor, Rust 1.98.1. Shared development host; measurements describe this fixture workload.

## Automated results

- `cargo test --locked`: **10 integration tests passed**, including 668 reference states/expected errors across 33 element sets.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- Release CLI, JSON/GeoJSON output, fixture HTTP server, and browser demo ran successfully.
- Browser checks: ISS epoch demo, 181 samples, antimeridian split, slider-selected position, stale-element warning, invalid time error, second satellite, 390px mobile viewport, and no JavaScript exceptions.

The suite checks the published SGP4 reference vectors (1e-6 km/component, 1e-9 km/s/component tolerances), separate analytic frame cases, application timestamp conversion, checksum/catalog errors, timezone normalization, request bounds, negative offsets, byte-identical repeated output, GeoJSON altitude units, provenance tampering, mocked provider HTTP/invalid-data/wrong-satellite failures, six-hour cache reuse, explicit stale fallback, immutable metadata, and versioned API errors.

## Warm 181-sample ISS track

Release build; 90 minutes from the fixture epoch at 30-second steps. Core component measurements average 1,000 iterations. HTTP measurement uses 200 requests after 10 warmups over a persistent loopback connection and excludes client JSON decoding. No provider requests are included.

| Measurement | Mean |
| --- | --- |
| SGP4 propagation only | 0.044 ms |
| Earth rotation + geodetic conversion | 0.080 ms |
| Full application track generation | 0.136 ms |
| JSON serialization only | 0.075 ms |
| HTTP end-to-end, including generation + serialization + transport | 0.458 ms |
| HTTP p95 | 0.720 ms |

The warm HTTP result meets the proposed 100 ms target for this workload. Component timings are separate measurements, not additive attribution of a single trace. Raw evidence: [core](core-measurement.json), [HTTP](http-measurement.json), [browser](browser-validation.json).

## Unverified boundary

Live CelesTrak requests were blocked in the implementation environment. Provider integration was tested with a local HTTP server, not the current production provider. Numerical agreement is with the chosen SGP4 model, not independent satellite telemetry. Ground-track Earth orientation is approximate and documented in `numerics.md`. GitHub CI is configured but has not run remotely.

## Visual refresh — 2026-10-27

Updated the explorer to match the personal website: white navigation, a purple night-sky background, indigo controls, and translucent cards. Replaced schematic outlines with an offline Natural Earth basemap, with correct spherical clipping at the date line. Added sampled playback and clearer time/coordinate readouts.

Revalidation: all 10 Rust integration tests passed (including the existing 668 reference states/failures); formatting and Clippy passed. Browser checks verified keyboard slider operation, play/pause and stopping at the final sample, GeoJSON download contents, stale warnings, invalid input, satellite switching, 320/390/768px layouts, reduced-motion behavior, and successful loading of local map/CSS assets. Desktop and mobile screenshots were inspected. No JavaScript exceptions or failed resource requests occurred.

The original performance measurements above remain the dated MVP measurements; the visual refresh does not change propagation or coordinate conversion.

## Separate website and Rust API — 2026-10-28

- All **13 integration tests** passed, including the original numerical suite and new tests for exact-origin CORS, explicit public binding, and the combined explorer response. The combined response matches the existing JSON/GeoJSON contracts. Formatting and Clippy passed; the release binary built successfully.
- The full desktop/mobile browser suite passed against a static frontend at `/orbit-explorer/` on port 8792 and the Rust API on port 8791. Actual browser CORS was exercised; no server-side proxy or browser security override was used.
- Additional browser checks passed for one prediction request per submission, initial connection failure and reconnect, a 503 preserving the prior map, and an unconfigured deployment without a localhost fallback. No JavaScript exceptions occurred.
- The Next.js website production build and type checks passed in an isolated staging copy. Its static export verified **12 HTML files and 234 local links/assets**. A browser followed the project page's launch button into the exported explorer and loaded 181 computed positions from the separate Rust API. Evidence: `website-integration.json` and `hosted-explorer*.png`.
- The static exporter rejected invalid backend origins and preserved the browser assets exactly.

The Docker image could not be built locally because this account cannot access the Docker daemon. CI now includes a container build, HTTP prediction check, and shutdown, but those remote checks have not run. Render deployment, public HTTPS/CORS, service startup latency, and GitHub Pages publication remain unverified. No backend host has been created. Provider data remains the labeled historical fixtures; the original performance measurements above were not repeated.

## Interactive observer tools and Rust reuse — 2026-10-28

- All **20 integration tests** passed, including the 668 SGP4 reference states/expected failures, independent observer-axis cases and WGS84 round trips, sampled pass boundaries/peaks, custom TLE validation, cache-key separation and eviction, all six bundled six-hour orbits, and JSON POST/CORS/body limits. Clippy and formatting passed.
- A real Chromium browser exercised the static frontend and Rust API on different loopback origins. It verified ground-station changes without new propagation, an overhead sample, pass selection, chart and keyboard navigation, map picking, 10-second resolution, 2,161-position windows, an orbital-period jump, custom TLE import/error recovery, a geostationary example, playback speeds, and 320/390/768px layouts. Evidence: `exploration-browser.json`, `explorer.png`, `explorer-mobile.png`, `observer-view.png`, and `explorer-showcase.png`.
- Existing desktop/mobile, GeoJSON download, stale/invalid-time, reduced-motion, startup/reconnect, and server-outage checks continued to pass. Browser screenshots were inspected and the mobile controls refined.
- The release benchmark compared 400 fresh/reused pairs per workload after 20 warmups. Median handler-plus-JSON time decreased by 40.7%, 43.3%, and 42.5% for 181, 721, and 2,161 positions. Complete numerical/geometry results matched exactly on every pair. See `performance.md` and `explorer-benchmark.json` for raw timings, source hashes, environment, and limits.
- The updated official website source passed its production build/type checks and static export checks (12 HTML files, 234 local links/assets). A browser verified the project performance description and new screenshot, followed the launch link, loaded six examples through the separate Rust API, and changed the observer with a cache hit. Evidence: `website-integration.json`.

These are local results. The user reports the previous Render service is live at `https://orbit-track-api.onrender.com`; this agent's network policy blocks live access to it and GitHub Pages. The upgraded POST endpoint, six-input container, and UI require a new deployment. Cloud startup, deployed latency, and the new Docker build remain unverified here. This project reports geometric sampled passes, not refined rise/set times or optical visibility.
