# Changelog

## Unreleased

- Support a GitHub Pages frontend connected to a separately hosted Rust API, with one server-generated track/GeoJSON response per update.
- Add exact-origin CORS configuration, explicit public binding, bounded concurrent predictions, connection recovery, and a Docker/Render deployment template.
- Verify separate-origin browser operation and website static-export integration; see `docs/hosting.md` for deployment steps.
- Match the personal website with white navigation, an indigo night-sky theme, and translucent panels.
- Replace schematic continents with an offline Natural Earth basemap.
- Add sampled-track playback, keyboard-accessible timeline controls, readable UTC/coordinate cards, and compact provenance details.
- Verify downloads, playback, responsive layouts, and reduced-motion behavior in the browser.

## 0.1.0 — 2026-10-26

Initial runnable MVP with fixture-driven validation, CLI, read-only local HTTP service, browser demo, documentation, and CI configuration. See README for scope and unverified live-integration limits.
