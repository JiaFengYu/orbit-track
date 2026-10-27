'use strict';

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
  }, 150);
}

async function get(url) {
  const response = await fetch(url);
  const data = await response.json();
  if (!response.ok) throw Error(data.error?.message ?? `HTTP ${response.status}`);
  return data;
}

async function predict(event) {
  event?.preventDefault();
  const token = ++request;
  stopPlayback();
  $('#run').disabled = true;
  $('#play').disabled = true;
  $('#error').textContent = '';
  $('#error').hidden = true;
  $('#status').textContent = 'Tracing the orbit…';
  $('#map').setAttribute('aria-busy', 'true');
  const query = new URLSearchParams({ start: $('#start').value.trim(), minutes: $('#duration').value, step_seconds: '30' });
  const url = `/v1/satellites/${$('#satellite').value}/track?${query}`;
  try {
    const [data, geo] = await Promise.all([get(url), get(`${url}&format=geojson`)]);
    if (token !== request) return;
    track = data;
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
    selectPoint();
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
    $('#status').textContent = `${data.satellite.name ?? data.satellite.catalog_id} · ${data.samples.length} positions · 30-second samples`;
  } catch (error) {
    if (token !== request) return;
    $('#error').textContent = error.message;
    $('#error').hidden = false;
    $('#status').textContent = track ? 'Prediction failed. The map still shows the previous successful track.' : 'No track to display. Check the request and try again.';
  } finally {
    if (token === request) {
      $('#run').disabled = false;
      $('#play').disabled = !track || track.samples.length < 2;
      $('#map').setAttribute('aria-busy', 'false');
    }
  }
}

$('#controls').addEventListener('submit', predict);
$('#slider').addEventListener('input', () => { stopPlayback(); selectPoint(); });
$('#play').addEventListener('click', togglePlayback);
$('#use-epoch').addEventListener('click', () => { $('#start').value = 'epoch'; $('#start').focus(); });
document.addEventListener('visibilitychange', () => { if (document.hidden) stopPlayback(); });

get('/v1/satellites').then(data => {
  for (const satellite of data.satellites) {
    const option = document.createElement('option');
    option.value = satellite.catalog_id;
    option.textContent = `${satellite.name ?? 'Satellite'} (${satellite.catalog_id})`;
    $('#satellite').append(option);
  }
  if (data.satellites.some(satellite => satellite.catalog_id === 25544)) $('#satellite').value = '25544';
  return predict();
}).catch(error => {
  $('#error').textContent = error.message;
  $('#error').hidden = false;
  $('#status').textContent = 'Catalog unavailable';
  $('#map-satellite').textContent = 'No satellite loaded';
  $('#run').disabled = true;
});
