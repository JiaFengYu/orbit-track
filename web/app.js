import { api } from './api-client.js';

const $ = selector => document.querySelector(selector);
const NS = 'http://www.w3.org/2000/svg';
const xy = (lon, lat) => [(lon + 180) / 360 * 1000, (90 - lat) / 180 * 500];
function svgNode(tag, attributes) {
  const element = document.createElementNS(NS, tag);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
  return element;
}

for (let lon = -180; lon <= 180; lon += 30) {
  const x = xy(lon, 0)[0];
  $('#grid').append(svgNode('line', { x1: x, x2: x, y1: 0, y2: 500, class: 'grid' }));
}
for (let lat = -60; lat <= 60; lat += 30) {
  const y = xy(0, lat)[1];
  $('#grid').append(svgNode('line', { x1: 0, x2: 1000, y1: y, y2: y, class: `grid${lat === 0 ? ' grid-equator' : ''}` }));
}
for (const [name, lon, lat, ocean] of [
  ['NORTH AMERICA', -106, 43], ['SOUTH AMERICA', -59, -18],
  ['EUROPE', 20, 51], ['AFRICA', 17, 9], ['ASIA', 89, 43], ['AUSTRALIA', 134, -25],
  ['PACIFIC OCEAN', -143, -9, true], ['ATLANTIC', -33, 17, true], ['INDIAN OCEAN', 76, -23, true],
]) {
  const [x, y] = xy(lon, lat);
  const label = svgNode('text', { x, y, class: `map-label${ocean ? ' ocean-label' : ''}` });
  label.textContent = name;
  $('#map-labels').append(label);
}

const dateFormatter = new Intl.DateTimeFormat('en', { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC' });
const clock = timestamp => new Date(timestamp).toISOString().slice(11, 19);
const date = timestamp => dateFormatter.format(new Date(timestamp));
const utc = timestamp => `${date(timestamp)}, ${clock(timestamp)} UTC`;
const coordinate = (degrees, positive, negative) => `${Math.abs(degrees).toFixed(2)}° ${degrees < 0 ? negative : positive}`;
let track = null;
let request = 0;
let downloadUrl = null;
let playTimer = null;
let observation = null;
let orbitalSummary = null;
let customTle = null;
const stations = { waterloo: [43.4643, -80.5204], sanfrancisco: [37.7749, -122.4194], london: [51.5074, -0.1278], tokyo: [35.6762, 139.6503], sydney: [-33.8688, 151.2093] };

function stopPlayback() {
  clearInterval(playTimer);
  playTimer = null;
  $('#play').setAttribute('aria-pressed', 'false');
  $('#play').setAttribute('aria-label', 'Play through predicted positions');
  $('#play-label').textContent = 'Play';
}

function selectPoint() {
  if (!track) return;
  const index = Number($('#slider').value);
  const sample = track.samples[index];
  const [x, y] = xy(sample.longitude_deg, sample.latitude_deg);
  for (const id of ['marker', 'marker-ring', 'marker-glow']) {
    const marker = $(`#${id}`);
    marker.setAttribute('cx', x);
    marker.setAttribute('cy', y);
    marker.setAttribute('visibility', 'visible');
  }
  $('#time').textContent = clock(sample.timestamp_utc);
  $('#date').textContent = date(sample.timestamp_utc);
  $('#coordinates').textContent = `${coordinate(sample.latitude_deg, 'N', 'S')} / ${coordinate(sample.longitude_deg, 'E', 'W')}`;
  $('#altitude').textContent = `${sample.altitude_km.toFixed(2)} km`;
  $('#age').textContent = `${Math.abs(sample.element_age_hours).toFixed(2)} hours`;
  $('#age-detail').textContent = sample.element_age_hours === 0 ? 'At the element epoch' : `${sample.element_age_hours < 0 ? 'Before' : 'After'} the element epoch`;
  $('#sample-position').textContent = `Position ${index + 1} of ${track.samples.length}`;
  $('#slider').style.setProperty('--progress', `${index / Math.max(1, track.samples.length - 1) * 100}%`);
  $('#slider').setAttribute('aria-valuetext', `${utc(sample.timestamp_utc)}, position ${index + 1} of ${track.samples.length}`);
  $('#map-title').textContent = `${track.satellite.name ?? track.satellite.catalog_id}: predicted position at ${utc(sample.timestamp_utc)}`;
  if (observation) {
    const look = observation.look_angles[index];
    $('#elevation').textContent = `${look.elevation_deg.toFixed(1)}°`;
    $('#azimuth').textContent = look.azimuth_deg === null ? (look.elevation_deg >= 0 ? 'At zenith' : '—') : `${look.azimuth_deg.toFixed(1)}°`;
    $('#range').textContent = `${Math.round(look.range_km).toLocaleString()} km`;
    $('#visibility').textContent = look.above_threshold ? `Above ${observation.observer.min_elevation_deg}° clearance` : `Below ${observation.observer.min_elevation_deg}° clearance`;
    $('#visibility').classList.toggle('above', look.above_threshold);
    const cursorX = index / Math.max(1, track.samples.length - 1) * 800;
    $('#chart-cursor').setAttribute('x1', cursorX);
    $('#chart-cursor').setAttribute('x2', cursorX);
    const skyPoint = $('#sky-point');
    skyPoint.setAttribute('visibility', look.elevation_deg >= 0 ? 'visible' : 'hidden');
    const radius = (90 - look.elevation_deg) / 90 * 65;
    const azimuth = (look.azimuth_deg ?? 0) * Math.PI / 180;
    skyPoint.setAttribute('cx', 90 + Math.sin(azimuth) * radius);
    skyPoint.setAttribute('cy', 90 - Math.cos(azimuth) * radius);
    $('#sky-note').textContent = look.elevation_deg >= 0 ? 'Center = overhead' : 'Below the geometric horizon';
  }
}

function jumpTo(index) {
  stopPlayback();
  if (!track) return;
  $('#slider').value = Math.max(0, Math.min(track.samples.length - 1, index));
  selectPoint();
}

function drawObservation() {
  const [x,y] = xy(observation.observer.longitude_deg, observation.observer.latitude_deg);
  $('#station-marker').setAttribute('transform', `translate(${x},${y})`);
  $('#station-marker').setAttribute('visibility', 'visible');
  const path = observation.look_angles.map((look,index) => `${index ? 'L' : 'M'}${index / Math.max(1, track.samples.length - 1) * 800},${10 + (90 - look.elevation_deg) / 180 * 160}`).join(' ');
  $('#elevation-line').setAttribute('d', path);
  const threshold = 10 + (90 - observation.observer.min_elevation_deg) / 180 * 160;
  $('#threshold-line').setAttribute('d', `M0 ${threshold}H800`);
  $('#chart-start').textContent = `${clock(track.samples[0].timestamp_utc)} UTC`;
  $('#chart-end').textContent = `${clock(track.samples.at(-1).timestamp_utc)} UTC`;
  $('#pass-count').textContent = `${observation.passes.length} sampled window${observation.passes.length === 1 ? '' : 's'}`;
  $('#passes').replaceChildren();
  for (const pass of observation.passes) {
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'chip';
    button.textContent = `${clock(track.samples[pass.first_index].timestamp_utc)}–${clock(track.samples[pass.last_index].timestamp_utc)} UTC`;
    const detail = document.createElement('span');
    detail.textContent = `Peak ${pass.max_elevation_deg.toFixed(1)}°${pass.clipped_start || pass.clipped_end ? ' · clipped by time window' : ''}`;
    button.append(detail);
    button.addEventListener('click', () => jumpTo(pass.peak_index));
    $('#passes').append(button);
  }
  if (!observation.passes.length) {
    const note = document.createElement('p');
    note.textContent = 'No sampled positions clear this angle. Try six hours, a different location, or a lower clearance.';
    $('#passes').append(note);
  }
}

function showPerformance(perf, roundtrip) {
  const ms = value => `${value < 0.001 ? '<0.001' : value.toFixed(3)} ms`;
  $('#compute-ms').textContent = ms(perf.compute_ms);
  $('#orbit-ms').textContent = ms(perf.orbit_ms);
  $('#observer-ms').textContent = ms(perf.observer_ms);
  $('#roundtrip-ms').textContent = ms(roundtrip);
  $('#reuse-note').textContent = perf.cache === 'hit'
    ? `Reused ${perf.returned_samples.toLocaleString()} orbit positions · 0 new SGP4 steps. Rust recalculated the observer geometry for this location.`
    : `Computed ${perf.propagated_samples.toLocaleString()} orbit positions in Rust.${perf.cache === 'bypass' ? ' Custom TLE results are not cached.' : ' Change the ground station to reuse this orbit.'}`;
  $('#reuse-note').dataset.cache = perf.cache;
}

function togglePlayback() {
  if (playTimer) { stopPlayback(); return; }
  if (!track || track.samples.length < 2) return;
  if (Number($('#slider').value) >= track.samples.length - 1) { $('#slider').value = 0; selectPoint(); }
  $('#play').setAttribute('aria-pressed', 'true');
  $('#play').setAttribute('aria-label', 'Pause predicted positions');
  $('#play-label').textContent = 'Pause';
  // One existing sample per tick; no interpolation or extra propagated states.
  playTimer = setInterval(() => {
    $('#slider').value = Math.min(Number($('#slider').value) + 1, track.samples.length - 1);
    selectPoint();
    if (Number($('#slider').value) === track.samples.length - 1) stopPlayback();
  }, 150 / Number($('#speed').value));
}

async function predict(event, override = null) {
  event?.preventDefault();
  if (!$('#controls').reportValidity() || !$('#station-controls').reportValidity()) return;
  const token = ++request;
  stopPlayback();
  $('#run').disabled = true;
  $('#play').disabled = true;
  $('#load-tle').disabled = true;
  $('#error').textContent = '';
  $('#error').hidden = true;
  $('#status').textContent = 'Tracing the orbit…';
  $('#map').setAttribute('aria-busy', 'true');
  const source = override ?? ($('#satellite').value === 'custom' ? { tle: customTle } : { catalog_id: Number($('#satellite').value) });
  const payload = { ...source, start: override ? 'epoch' : $('#start').value.trim(), minutes: Number($('#duration').value), step_seconds: Number($('#resolution').value), observer: { latitude_deg: Number($('#station-lat').value), longitude_deg: Number($('#station-lon').value), altitude_km: 0, min_elevation_deg: Number($('#min-elevation').value) } };
  try {
    const sent = performance.now();
    const { track: data, geojson: geo, observation: analysis, performance: perf, orbit } = await api.explore(payload);
    const roundtrip = performance.now() - sent;
    if (token !== request) return;
    if (override) {
      customTle = override.tle;
      let option = $('#satellite option[value=custom]');
      if (!option) { option = document.createElement('option'); option.value = 'custom'; $('#satellite').append(option); }
      option.textContent = `${data.satellite.name ?? data.satellite.catalog_id} · your TLE`;
      $('#satellite').value = 'custom'; $('#start').value = 'epoch';
      $('#tle-error').hidden = true;
    }
    track = data;
    observation = analysis;
    orbitalSummary = orbit;
    $('#track').replaceChildren();
    $('#track-glow').replaceChildren();
    for (const line of geo.geometry.coordinates) {
      const path = svgNode('polyline', { points: line.map(point => xy(point[0], point[1]).join(',')).join(' ') });
      $('#track').append(path);
      $('#track-glow').append(path.cloneNode());
    }
    $('#map-satellite').textContent = data.satellite.name ?? `Satellite ${data.satellite.catalog_id}`;
    $('#slider').max = track.samples.length - 1;
    $('#slider').value = 0;
    $('#slider').disabled = false;
    $('#timeline-start').textContent = `${clock(data.samples[0].timestamp_utc)} UTC`;
    $('#timeline-end').textContent = `${clock(data.samples.at(-1).timestamp_utc)} UTC`;
    $('#step-label').textContent = `${data.request.step_seconds}-second samples`;
    $('#orbit-summary').textContent = `${orbit.period_minutes.toFixed(1)} min orbit · ${orbit.inclination_deg.toFixed(1)}° inclination · e ${orbit.eccentricity.toFixed(4)}`;
    $('#next-orbit').disabled = false;
    drawObservation();
    selectPoint();
    showPerformance(perf, roundtrip);
    $('#provenance').textContent = data.elements.provider;
    $('#element-epoch').textContent = utc(data.elements.epoch_utc);
    $('#element-epoch').title = data.elements.epoch_utc;
    $('#fetched').textContent = data.elements.fetched_at_utc ? utc(data.elements.fetched_at_utc) : 'Local element file';
    $('#model').textContent = `SGP4 ${data.model.version} · ${data.model.gravity_model} · ${data.model.initialization_mode} · geodetic ${data.coordinates.geodetic_datum}`;
    $('#hash').textContent = `SHA-256 ${data.elements.sha256}`;
    // The approximation is always explained in the adjacent context paragraph.
    const actionableWarnings = data.warnings.filter(warning => warning.code !== 'APPROXIMATE_EARTH_ORIENTATION');
    $('#warnings').replaceChildren(...actionableWarnings.map(warning => {
      const paragraph = document.createElement('p'); paragraph.textContent = warning.message; return paragraph;
    }));
    if (downloadUrl) URL.revokeObjectURL(downloadUrl);
    downloadUrl = URL.createObjectURL(new Blob([JSON.stringify(geo, null, 2)], { type: 'application/geo+json' }));
    $('#download').href = downloadUrl;
    $('#download').setAttribute('aria-disabled', 'false');
    $('#download').removeAttribute('tabindex');
    $('#status').textContent = `${data.satellite.name ?? data.satellite.catalog_id} · ${data.samples.length} positions · ${data.request.step_seconds}-second samples`;
  } catch (error) {
    if (token !== request) return;
    $('#error').textContent = error.message;
    $('#error').hidden = false;
    if (override) { $('#tle-error').textContent = error.message; $('#tle-error').hidden = false; }
    $('#status').textContent = track ? 'Prediction failed. The map still shows the previous successful track.' : 'No track to display. Check the request and try again.';
  } finally {
    if (token === request) {
      $('#run').disabled = false;
      $('#load-tle').disabled = false;
      $('#play').disabled = !track || track.samples.length < 2;
      $('#map').setAttribute('aria-busy', 'false');
    }
  }
}

$('#controls').addEventListener('submit', predict);
$('#station-controls').addEventListener('submit', predict);
$('#slider').addEventListener('input', () => { stopPlayback(); selectPoint(); });
$('#play').addEventListener('click', togglePlayback);
$('#speed').addEventListener('change', () => { if (playTimer) { stopPlayback(); togglePlayback(); } });
$('#satellite').addEventListener('change', () => { $('#start').value = 'epoch'; });
$('#long-window').addEventListener('click', () => { $('#duration').value = 360; predict(); });
$('#next-orbit').addEventListener('click', () => {
  if (track && orbitalSummary) { $('#start').value = new Date(Date.parse(track.request.start_utc) + orbitalSummary.period_minutes * 60000).toISOString(); predict(); }
});
$('#station').addEventListener('change', () => {
  const coordinates = stations[$('#station').value];
  if (coordinates) { [$('#station-lat').value, $('#station-lon').value] = coordinates; predict(); }
});
for (const id of ['station-lat', 'station-lon']) $(`#${id}`).addEventListener('input', () => { $('#station').value = 'custom'; });
$('#pick-location').addEventListener('click', () => {
  const picking = $('#pick-location').getAttribute('aria-pressed') !== 'true';
  $('#pick-location').setAttribute('aria-pressed', String(picking));
  $('#pick-location').textContent = picking ? 'Click a place on the map…' : 'Pick on the map';
  $('#map').classList.toggle('picking', picking);
});
$('#map').addEventListener('click', event => {
  if ($('#pick-location').getAttribute('aria-pressed') !== 'true') return;
  const point = new DOMPoint(event.clientX, event.clientY).matrixTransform($('#map').getScreenCTM().inverse());
  $('#station-lat').value = Math.max(-90, Math.min(90, 90 - point.y / 500 * 180)).toFixed(4);
  $('#station-lon').value = Math.max(-180, Math.min(180, point.x / 1000 * 360 - 180)).toFixed(4);
  $('#station').value = 'custom'; $('#pick-location').click(); predict();
});
$('#elevation-chart').addEventListener('click', event => {
  if (!track) return;
  const point = new DOMPoint(event.clientX, event.clientY).matrixTransform($('#elevation-chart').getScreenCTM().inverse());
  jumpTo(Math.round(point.x / 800 * (track.samples.length - 1)));
});
$('#load-tle').addEventListener('click', () => {
  const tle = $('#tle-input').value.trim();
  if (!tle) { $('#tle-error').textContent = 'Paste a TLE first.'; $('#tle-error').hidden = false; return; }
  predict(null, { tle: tle + '\n' });
});
$('#use-epoch').addEventListener('click', () => { $('#start').value = 'epoch'; $('#start').focus(); });
document.addEventListener('visibilitychange', () => { if (document.hidden) stopPlayback(); });

async function loadCatalog() {
  $('#retry').hidden = true;
  $('#run').disabled = true;
  $('#error').hidden = true;
  $('#status').textContent = 'Connecting to the orbit service… The first visit may take a moment.';
  try {
    const data = await api.catalog();
    $('#catalog-count').textContent = `${data.satellites.length} catalog examples · bring your own TLE`;
    $('#satellite').replaceChildren();
    for (const satellite of data.satellites) {
      const option = document.createElement('option');
      option.value = satellite.catalog_id;
      option.textContent = `${satellite.name ?? 'Satellite'} (${satellite.catalog_id})`;
      $('#satellite').append(option);
    }
    if (data.satellites.some(satellite => satellite.catalog_id === 25544)) $('#satellite').value = '25544';
    await predict();
  } catch (error) {
    $('#error').textContent = error.message;
    $('#error').hidden = false;
    $('#status').textContent = 'Catalog unavailable';
    $('#map-satellite').textContent = 'No satellite loaded';
    $('#run').disabled = true;
    $('#retry').hidden = false;
  }
}
$('#retry').addEventListener('click', loadCatalog);
loadCatalog();
