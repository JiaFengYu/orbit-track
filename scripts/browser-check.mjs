import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || undefined, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
const errors = [];
const failedResources = [];
page.on('pageerror', error => errors.push(error.message));
page.on('requestfailed', request => failedResources.push(request.url()));
const downloadDir = await mkdtemp(join(tmpdir(), 'orbit-download-'));
try {
  const base = process.env.DEMO_URL || 'http://127.0.0.1:8788';
  await page.goto(base);
  await page.waitForFunction(() => document.querySelector('#status').textContent.includes('181 positions'));
  await page.waitForLoadState('networkidle');
  assert.equal(await page.locator('#satellite').inputValue(), '25544');
  assert.ok(await page.locator('#track polyline').count() >= 2, 'ISS demo should cross the date line');
  assert.equal(await page.locator('.site-nav').evaluate(nav => getComputedStyle(nav).backgroundColor), 'rgb(255, 255, 255)');
  for (const [asset, contentType] of [['/styles.css', 'text/css'], ['/basemap.svg', 'image/svg+xml']]) {
    const response = await page.request.get(base + asset);
    assert.equal(response.status(), 200);
    assert.ok(response.headers()['content-type'].startsWith(contentType));
  }
  const before = await page.locator('#time').textContent();
  await page.locator('#slider').fill('90');
  await page.locator('#slider').dispatchEvent('input');
  assert.notEqual(await page.locator('#time').textContent(), before);
  await page.locator('#slider').focus();
  await page.keyboard.press('ArrowRight');
  assert.equal(await page.locator('#slider').inputValue(), '91');
  assert.ok((await page.locator('#slider').getAttribute('aria-valuetext')).includes('UTC'));
  await page.locator('#play').click();
  await page.waitForFunction(() => Number(document.querySelector('#slider').value) > 91);
  await page.locator('#play').click();
  const stopped = await page.locator('#slider').inputValue();
  await page.waitForTimeout(350);
  assert.equal(await page.locator('#slider').inputValue(), stopped, 'paused playback must stop');
  await page.locator('#slider').fill('179');
  await page.locator('#slider').dispatchEvent('input');
  await page.locator('#play').click();
  await page.waitForFunction(() => document.querySelector('#slider').value === '180' && document.querySelector('#play').getAttribute('aria-pressed') === 'false');
  const [download] = await Promise.all([page.waitForEvent('download'), page.locator('#download').click()]);
  await download.saveAs(join(downloadDir, 'track.geojson'));
  const geo = JSON.parse(await readFile(join(downloadDir, 'track.geojson'), 'utf8'));
  assert.equal(geo.properties.satellite.catalog_id, 25544);
  assert.equal(geo.geometry.type, 'MultiLineString');

  const epoch = await page.evaluate(async () => {
    const catalog = await (await fetch('/v1/satellites')).json();
    return catalog.satellites.find(satellite => satellite.catalog_id === 25544).epoch_utc;
  });
  await page.locator('#start').fill(new Date(Date.parse(epoch) + 4 * 86400000).toISOString());
  await page.locator('#run').click();
  await page.waitForFunction(() => document.querySelector('#warnings').textContent.includes('warning threshold 72 hours'));
  await page.locator('#start').fill('invalid');
  await page.locator('#run').click();
  await page.waitForFunction(() => document.querySelector('#error').textContent.includes('RFC3339'));
  assert.equal(await page.locator('#error').isVisible(), true);
  await page.locator('#use-epoch').click();
  assert.equal(await page.locator('#start').inputValue(), 'epoch');
  await page.locator('#satellite').selectOption('5');
  await page.locator('#run').click();
  await page.waitForFunction(() => document.querySelector('#status').textContent.includes('VANGUARD'));
  await page.locator('#satellite').selectOption('25544');
  await page.locator('#run').click();
  await page.waitForFunction(() => document.querySelector('#status').textContent.includes('ISS'));
  await page.locator('#slider').fill('90');
  await page.locator('#slider').dispatchEvent('input');
  await page.locator('#slider').evaluate(slider => slider.blur());
  await page.screenshot({ path: 'docs/explorer.png', fullPage: true });
  for (const width of [320, 390, 768]) {
    await page.setViewportSize({ width, height: 844 });
    await page.waitForTimeout(100);
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `horizontal overflow at ${width}px`);
    assert.ok((await page.locator('#altitude').textContent()).includes('km'));
  }
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: 'docs/explorer-mobile.png', fullPage: true });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  assert.equal(await page.locator('html').evaluate(html => getComputedStyle(html).scrollBehavior), 'auto');
  assert.deepEqual(errors, []);
  assert.deepEqual(failedResources, []);
  console.log(JSON.stringify({ passed: true, checks: ['ISS epoch demo', '181 positions', 'antimeridian split', 'offline map and stylesheet', 'white website navigation', 'keyboard slider', 'play/pause and end stop', 'GeoJSON download', 'stale warning', 'invalid time', 'epoch shortcut', 'satellite selection', '320/390/768px layouts', 'reduced motion', 'no JS errors or failed resources'] }));
} finally {
  await browser.close();
  await rm(downloadDir, { recursive: true, force: true });
}
