# Offline map asset

`land-110m.topo.json` is the land topology from **world-atlas 2.0.2**, a redistribution of **Natural Earth 4.1.0** at 1:110 million scale. It was obtained from the published npm package, not drawn by hand.

- Data source: https://www.naturalearthdata.com/downloads/110m-physical-vectors/110m-land/
- Distributor: https://www.npmjs.com/package/world-atlas/v/2.0.2
- Source SHA-256: `ead5f68119c49a9250902e7da303bcb209341bbb8fefe7369a439b48b704658a`
- Natural Earth data is public domain. The distributor's copyright/permission notice is preserved in `WORLD-ATLAS-LICENSE`.

`web/basemap.svg` is a generated equirectangular map. The build uses d3-geo's spherical clipping to avoid drawing coastline seams across the date line and to close polar rings correctly. Its projection matches the app's 1000×500 coordinate system.

To regenerate after changing data or styling, with Node installed:

```sh
cd tools/map
npm ci
npm run build
```

The pinned npm packages are build-time tools only. The Rust binary embeds the generated SVG, CSS, and application JavaScript; running the explorer needs no Node installation, map tiles, CDN, network connection, or JavaScript framework. The map credits Natural Earth in the UI.
