# Numerical conventions and fixture provenance

## Sources

The fixture inputs are copied verbatim from the MIT-licensed **sgp4 2.4.0 crate distribution**, retrieved through Cargo during implementation:

- `fixtures/iss.tle`: the two TLE lines and name from `examples/tle_afspc.rs` (July 2020 ISS elements).
- `fixtures/vanguard.tle`: first input of `tests/test_cases.toml`, with the descriptive name VANGUARD 1 added.
- `fixtures/vallado-reference.toml`: verbatim `tests/test_cases.toml`, 33 element sets and 668 expected states/failures.
- `fixtures/SGP4-LICENSE`: verbatim upstream MIT notice, copyright International Centre for Neuromorphic Systems.

Upstream: <https://crates.io/crates/sgp4/2.4.0>, <https://github.com/neuromorphicsystems/sgp4>.

The crate identifies its reference as Vallado, Crawford, Hujsak and Kelso, *Revisiting Spacetrack Report #3*, AIAA 2006-6753, <https://doi.org/10.2514/6.2006-6753>. Expected reference values are copied, not generated using our wrapper. The entire dataset checks initialization/model settings and propagation; a separate end-to-end Vanguard test checks timestamp-to-minutes conversion through the application's sample path. Very long reference cases exercise the underlying initialized propagator directly, outside the application's ±14-day product bound.

## Tolerances and coordinate checks

Reference-vector tolerances are fixed at **1e-6 km per position component** and **1e-9 km/s per velocity component**, matching the upstream verification tests. These are software comparison tolerances, not orbit prediction accuracy claims.

The frame-conversion tests are independent of the propagation reference checks:

- GMST at J2000 noon UTC: 280.46061837°; tolerance 1e-7°.
- A 90° Earth rotation maps TEME +X to Earth-fixed −Y.
- WGS84 equator/pole cases and an independent forward-ellipsoid position at 45°N, 30°E, 400 km altitude.
- General-coordinate inversion tolerance: 1e-9° latitude/longitude and 1e-6 km altitude.
- Bidirectional antimeridian crossings at high latitudes, with display altitude interpolation.

GMST uses the IAU-1982 polynomial in Julian centuries from J2000. UTC substitutes for UT1; polar motion is zero. The transformation is intentionally approximate. ECEF→geodetic uses an iterative WGS84 solution with an explicit polar case. Gravity constants (WGS72) and geodetic ellipsoid (WGS84) serve different purposes and are recorded separately.

## Reproducibility

Raw file bytes, including newline conventions, determine SHA-256. Metadata is checked against the raw bytes. Each response records epoch, digest, provider/source, fetch time, model version, frame conventions, and sample times. Identical files/requests under the pinned implementation serialize identically without fetching data. No date-of-generation field is injected into prediction output.

See `fixtures/provenance.json` for the raw fixture digests and upstream references. These are historical fixtures; use `--start epoch` for the default demo.
