import assert from 'node:assert/strict';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || undefined, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage'] });
const base = process.env.DEMO_URL || 'http://127.0.0.1:8792/orbit-explorer/';
const api = process.env.API_URL || 'http://127.0.0.1:8791';
const context = await browser.newContext();
const page = await context.newPage();
const errors = [];
const apiRequests = [];
page.on('pageerror', error => errors.push(error.message));
page.on('request', request => { if (new URL(request.url()).pathname.startsWith('/v1/')) apiRequests.push(request.url()); });
try {
  // Actual CORS path: the browser and Rust server are different origins.
  await page.goto(base);
  await page.waitForFunction(() => document.querySelector('#status').textContent.includes('181 positions'));
  assert.equal(apiRequests.length, 2, 'one catalog request and one combined prediction');
  assert.ok(apiRequests.every(url => new URL(url).origin === new URL(api).origin));
  assert.equal(new URL(apiRequests[1]).pathname, '/v1/explore');
  const before = apiRequests.length;
  await page.locator('#duration').fill('30');
  await page.locator('#run').click();
  await page.waitForFunction(() => document.querySelector('#status').textContent.includes('61 positions'));
  assert.equal(apiRequests.length, before + 1, 'one server calculation per control submission');

  // A failed initial connection offers recovery, without a reload or host details.
  await page.route(`${api}/v1/**`, route => route.abort());
  await page.reload();
  await page.waitForSelector('#retry', { state: 'visible' });
  assert.ok((await page.locator('#error').textContent()).includes('could not be reached'));
  await page.unroute(`${api}/v1/**`);
  await page.locator('#retry').click();
  await page.waitForFunction(() => document.querySelector('#status').textContent.includes('181 positions'));
  assert.equal(await page.locator('#retry').isVisible(), false);

  // A later outage preserves the last successful map and permits a retry.
  await page.route(`${api}/v1/**`, route => route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: { message: 'The explorer is busy. Please try again shortly.' } }) }));
  await page.locator('#run').click();
  await page.waitForFunction(() => document.querySelector('#error').textContent.includes('busy'));
  assert.ok((await page.locator('#status').textContent()).includes('previous successful track'));
  assert.equal(await page.locator('#run').isEnabled(), true);
  assert.ok(await page.locator('#track polyline').count() > 0);
  await page.unroute(`${api}/v1/**`);

  // Static deploys with no backend configured do not silently use localhost.
  await page.route('**/config.js', route => route.fulfill({ contentType: 'text/javascript', body: 'export const apiBase = null;' }));
  const requestsBefore = apiRequests.length;
  await page.reload();
  await page.waitForSelector('#retry', { state: 'visible' });
  assert.ok((await page.locator('#error').textContent()).includes('not available yet'));
  assert.equal(apiRequests.length, requestsBefore);
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ passed: true, checks: ['separate static frontend and Rust origins', 'real browser CORS', 'one combined prediction request', 'initial failure and reconnect', 'busy server preserves prior map', 'unconfigured deployment message', 'no browser exceptions'] }));
} finally {
  await browser.close();
}
