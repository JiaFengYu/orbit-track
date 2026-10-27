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
