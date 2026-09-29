import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { parseEnv } from 'node:util';
import { chromium } from '@playwright/test';
const base=(process.argv[2]||'').replace(/\/$/,'');
const url=new URL(base);
assert.equal(url.protocol,'https:');
assert.match(url.hostname,/^valorant-rank-tracker\.[a-z\d-]+\.workers\.dev$/);
const token=parseEnv(await readFile(new URL('../.dev.vars',import.meta.url),'utf8')).ADMIN_TOKEN;
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const report={host:base,started_at:new Date().toISOString(),checks:[]};
async function get(path){const r=await fetch(`${base}${path}`,{signal:AbortSignal.timeout(30000)});assert.equal(r.status,200,path);return r.json();}
async function admin(action){for(let i=0;i<10;i++){const r=await fetch(`${base}/api/admin/${action}`,{method:'POST',headers:{Authorization:`Bearer ${token}`},signal:AbortSignal.timeout(30000)});if(r.status===409){await delay(1000);continue;}assert.equal(r.status,200,action);return r.json();}throw Error('Refresh remained busy');}
let state;
for(let i=0;i<12;i++){try{state=await get('/api/state');break;}catch(e){if(i===11)throw e;await delay(5000);}}
assert.equal(state.meta.source,'henrikdev');assert.ok(state.rank.tier_name);assert.ok(state.rank.icon);report.rank=state.rank;
let health=await get('/api/health');assert.equal(health.mode,'live');assert.equal(health.refresh_secs,60);assert.equal(health.watch_secs,600);assert.ok(health.next_alarm);report.checks.push('live account, production cadence, next alarm');
assert.equal((await fetch(`${base}/api/admin/refresh`,{method:'POST'})).status,401);
const refreshed=await admin('refresh');assert.equal(refreshed.refreshed,true);report.checks.push('admin denies missing token, accepts configured secret');
const rank=await fetch(`${base}/api/rank.txt`);assert.equal(rank.headers.get('cache-control'),'no-store');assert.match(await rank.text(),/RR/);
for(const path of ['/overlay/rank-card','/overlay/history','/overlay/session','/overlay/alerts','/dashboard/','/fonts/anton.woff2','/fonts/rajdhani.woff2'])assert.equal((await fetch(`${base}${path}`)).status,200,path);
report.checks.push('rank text, overlays, dashboard, self-hosted fonts');
console.log(`Cloud live rank: ${state.rank.tier_name}, ${state.rank.rr} RR`);
const checked=refreshed.state.meta.checked_at;
// Observe the production alarm and initial import rather than accelerating it.
for(let i=0;i<8;i++){
  await delay(60000);
  state=await get('/api/state');health=await get('/api/health');
  console.log(`Cloud alarm observed: ${state.history.length} matches, catch-up ${state.meta.catching_up}, stale ${state.meta.stale}`);
  assert.equal(state.meta.stale,false);assert.equal(health.backoff.step,0);
  if(state.meta.checked_at!==checked&&!state.meta.catching_up)break;
  if(i===7)throw Error('Cloud history import did not finish within eight alarm cycles');
}
assert.notEqual(state.meta.checked_at,checked);assert.equal(state.meta.catching_up,false);
report.checks.push('production 60-second alarm, complete quiet history import, no stale state');report.history_count=state.history.length;report.session=state.session;
const browser=await chromium.launch({headless:true});
try{
  const page=await browser.newPage({viewport:{width:1440,height:1100}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`${base}/dashboard/`);await page.locator('#rank').filter({hasText:state.rank.tier_name}).waitFor();assert.equal(await page.locator('#mode').innerText(),'LIVE / AP');
  await mkdir(new URL('../artifacts',import.meta.url),{recursive:true});await page.screenshot({path:fileURLToPath(new URL('../artifacts/cloud-dashboard.png',import.meta.url)),fullPage:true});
  await page.goto(`${base}/overlay/rank-card`);await page.locator('.rank-top h1').filter({hasText:state.rank.tier_name}).waitFor();await page.screenshot({path:fileURLToPath(new URL('../artifacts/cloud-rank-card.png',import.meta.url)),clip:{x:0,y:0,width:520,height:160}});
  await page.goto(`${base}/overlay/history`);await page.locator('.match-chip').first().waitFor();
  await page.goto(`${base}/overlay/session`);await page.locator('svg.chart').waitFor();
  await page.goto(`${base}/overlay/alerts?poll=5`);await delay(1200);await admin('test/win');await page.locator('.alert-title').filter({hasText:'VICTORY'}).waitFor({timeout:15000});
  assert.equal(errors.length,0,errors.join('; '));report.checks.push('cloud browser dashboard, rank, history, session, injected win alert');
}finally{await browser.close();}
report.finished_at=new Date().toISOString();await writeFile(new URL('../artifacts/cloud-verification.json',import.meta.url),JSON.stringify(report,null,2));
console.log('Cloud verification passed with local dev server stopped');
