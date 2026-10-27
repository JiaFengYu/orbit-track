// Build-time tools only. The app serves the resulting SVG without JS dependencies.
import { readFile, writeFile } from 'node:fs/promises';
import { geoEquirectangular, geoPath } from 'd3-geo';
import { feature } from 'topojson-client';

const topology = JSON.parse(await readFile(new URL('../../assets/land-110m.topo.json', import.meta.url), 'utf8'));
const land = feature(topology, topology.objects.land);
const projection = geoEquirectangular().scale(1000 / (2 * Math.PI)).translate([500, 250]).precision(0.1);
// d3-geo clips spherical geometry at the antimeridian and closes polar rings.
const path = geoPath(projection)(land);
const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500">
<!-- Natural Earth 4.1.0 / world-atlas 2.0.2. See assets/README.md and WORLD-ATLAS-LICENSE. -->
<path fill="#353658" stroke="#767a9d" stroke-opacity=".55" stroke-width=".45" stroke-linejoin="round" d="${path}"/>
</svg>
`;
await writeFile(new URL('../../web/basemap.svg', import.meta.url), svg);
console.log(`Wrote offline basemap (${Buffer.byteLength(svg)} bytes).`);
