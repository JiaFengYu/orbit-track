import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || undefined, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
const exceptions = [];
page.on('pageerror', error => exceptions.push(error.message));
async function act(action) {
  const pending = page.waitForResponse(r => new URL(r.url()).pathname === '/v1/explore' && r.request().method() === 'POST');
  await action();
  const response = await pending;
  const body = await response.json();
  await page.waitForFunction(() => document.querySelector('#map').getAttribute('aria-busy') === 'false');
  return { response, body };
}
try {
  await page.goto(process.env.DEMO_URL || 'http://127.0.0.1:8792/orbit-explorer/');
  await page.waitForFunction(() => document.querySelector('#status').textContent.includes('181 positions'));
  assert.equal(await page.locator('#satellite option').count(), 6);
  assert.ok((await page.locator('#compute-ms').textContent()).includes('ms'));
  const moved = await act(() => page.locator('#station').selectOption('tokyo'));
  assert.equal(moved.body.performance.cache, 'hit');
  assert.equal(moved.body.performance.propagated_samples, 0);
  assert.equal(moved.body.observation.observer.latitude_deg, 35.6762);
  assert.ok((await page.locator('#reuse-note').textContent()).includes('0 new SGP4 steps'));

  const overhead = moved.body.track.samples[90];
  await page.locator('#station-lat').fill(String(overhead.latitude_deg));
  await page.locator('#station-lon').fill(String(overhead.longitude_deg));
  const visible = await act(() => page.locator('#station-controls button[type=submit]').click());
  const passIndex = visible.body.observation.passes.findIndex(pass => pass.peak_index === 90);
  assert.ok(passIndex >= 0, 'station beneath a sample should have an overhead pass');
  await page.locator('#passes button').nth(passIndex).click();
  assert.equal(await page.locator('#slider').inputValue(), '90');
  assert.equal(await page.locator('#elevation').textContent(), '90.0°');
  assert.ok((await page.locator('#visibility').textContent()).startsWith('Above'));
  const chart = await page.locator('#elevation-chart').boundingBox();
  await page.locator('#elevation-chart').click({ position: { x: chart.width * .25, y: chart.height * .5 } });
  assert.ok(Math.abs(Number(await page.locator('#slider').inputValue()) - 45) <= 1);
  await page.locator('#slider').focus();
  await page.keyboard.press('ArrowRight');
  assert.ok((await page.locator('#slider').getAttribute('aria-valuetext')).includes('UTC'));

  await page.locator('#pick-location').click();
  const map = await page.locator('#map').boundingBox();
  const picked = await act(() => page.locator('#map').click({ position: { x: map.width * .3, y: map.height * .4 } }));
  assert.ok(Math.abs(picked.body.observation.observer.latitude_deg - 18) < .5);
  assert.ok(Math.abs(picked.body.observation.observer.longitude_deg + 72) < .5);
  assert.equal(await page.locator('#station').inputValue(), 'custom');
  assert.equal(await page.locator('#pick-location').getAttribute('aria-pressed'), 'false');

  await page.locator('#resolution').selectOption('10');
  const dense = await act(() => page.locator('#run').click());
  assert.equal(dense.body.track.samples.length, 541);
  const long = await act(() => page.locator('#long-window').click());
  assert.equal(long.body.track.samples.length, 2161);
  const next = await act(() => page.locator('#next-orbit').click());
  assert.ok(Date.parse(next.body.track.request.start_utc) > Date.parse(long.body.track.request.start_utc));

  await page.locator('.import-panel summary').click();
  await page.locator('#tle-input').fill(await readFile('fixtures/polar.tle', 'utf8'));
  const custom = await act(() => page.locator('#load-tle').click());
  assert.equal(custom.body.track.satellite.catalog_id, 28057);
  assert.equal(custom.body.performance.cache, 'bypass');
  assert.equal(await page.locator('#satellite').inputValue(), 'custom');
  await page.locator('#tle-input').fill('broken TLE');
  const bad = await act(() => page.locator('#load-tle').click());
  assert.equal(bad.response.status(), 400);
  assert.equal(await page.locator('#tle-error').isVisible(), true);
  assert.ok((await page.locator('#status').textContent()).includes('previous successful track'));
  await page.locator('.import-panel summary').click();

  await page.locator('#satellite').selectOption('25954');
  const geo = await act(() => page.locator('#run').click());
  assert.ok(geo.body.orbit.period_minutes > 1400);
  await page.locator('#speed').selectOption('4');
  await page.locator('#play').click();
  await page.waitForFunction(() => Number(document.querySelector('#slider').value) >= 4);
  await page.locator('#play').click();

  await page.locator('#satellite').selectOption('25544');
  await page.locator('#duration').fill('180');
  await page.locator('#resolution').selectOption('30');
  const final = await act(() => page.locator('#run').click());
  const sample = final.body.track.samples[90];
  await page.locator('#station-lat').fill(sample.latitude_deg.toFixed(4));
  await page.locator('#station-lon').fill(sample.longitude_deg.toFixed(4));
  await act(() => page.locator('#station-controls button[type=submit]').click());
  await page.locator('#slider').fill('90');
  await page.locator('#slider').dispatchEvent('input');
  await page.locator('#slider').evaluate(el => el.blur());
  await page.screenshot({ path: 'docs/explorer.png', fullPage: true });
  await page.locator('.observer-view').screenshot({ path: 'docs/observer-view.png' });
  const mapBox = await page.locator('.explorer').boundingBox();
  const observerBox = await page.locator('.observer-view').boundingBox();
  const scrollY = await page.evaluate(() => window.scrollY);
  await page.screenshot({ path: 'docs/explorer-showcase.png', fullPage: true, clip: { x: mapBox.x, y: mapBox.y + scrollY, width: mapBox.width, height: observerBox.y + observerBox.height - mapBox.y } });
  for (const width of [320, 390, 768]) {
    await page.setViewportSize({ width, height: 844 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `overflow at ${width}`);
  }
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: 'docs/explorer-mobile.png', fullPage: true });
  assert.deepEqual(exceptions, []);
  const evidence = { passed: true, checks: ['six catalog examples', 'live Rust timings', 'observer change reuses SGP4', 'overhead geometry', 'pass jump', 'interactive elevation chart', 'keyboard slider', 'map location picking', 'sample resolution', 'six-hour window', 'advance one orbit', 'custom TLE import and invalid input recovery', 'geostationary example', 'playback speed', '320/390/768px layouts', 'no browser exceptions'] };
  await writeFile('docs/exploration-browser.json', JSON.stringify(evidence, null, 2) + '\n');
  console.log(JSON.stringify(evidence));
} finally { await browser.close(); }
