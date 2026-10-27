import assert from 'node:assert/strict';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({executablePath:process.env.CHROME_BIN || undefined,headless:true,args:['--no-sandbox','--disable-dev-shm-usage']});
const page = await browser.newPage({viewport:{width:1280,height:900}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
try {
  await page.goto(process.env.DEMO_URL || 'http://127.0.0.1:8788');
  await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('181 positions'));
  assert.equal(await page.locator('#satellite').inputValue(),'25544');
  assert.ok(await page.locator('#track polyline').count()>=2,'ISS demo should cross the date line');
  const before=await page.locator('#time').textContent();
  await page.locator('#slider').fill('90');await page.locator('#slider').dispatchEvent('input');
  assert.notEqual(await page.locator('#time').textContent(),before);
  await page.screenshot({path:'docs/explorer.png',fullPage:true});
  const epoch=await page.evaluate(async()=>{const c=await(await fetch('/v1/satellites')).json();return c.satellites.find(s=>s.catalog_id===25544).epoch_utc});
  await page.locator('#start').fill(new Date(Date.parse(epoch)+4*86400000).toISOString());await page.locator('#run').click();
  await page.waitForFunction(()=>document.querySelector('#warnings').textContent.includes('warning threshold 72 hours'));
  await page.locator('#start').fill('invalid');await page.locator('#run').click();await page.waitForFunction(()=>document.querySelector('#error').textContent.includes('RFC3339'));
  await page.locator('#start').fill('epoch');await page.locator('#satellite').selectOption('5');await page.locator('#run').click();await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('VANGUARD'));
  await page.locator('#satellite').selectOption('25544');await page.locator('#run').click();await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('ISS'));
  await page.setViewportSize({width:390,height:844});
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'mobile horizontal overflow');
  await page.screenshot({path:'docs/explorer-mobile.png',fullPage:true});
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({passed:true,checks:['ISS epoch demo','181 positions','antimeridian split','slider','stale warning','invalid time','satellite selection','390px viewport','no JS errors']}));
} finally {await browser.close();}
