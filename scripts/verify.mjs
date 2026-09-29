import { spawn, execFileSync } from 'node:child_process';
import { mkdir, readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import { chromium } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import { parseEnv } from 'node:util';
const root=new URL('../',import.meta.url);
const cli=new URL('../node_modules/wrangler/bin/wrangler.js',import.meta.url);
const base='http://127.0.0.1:8788';
const token=parseEnv(await readFile(new URL('../.dev.vars',import.meta.url),'utf8')).ADMIN_TOKEN;
if(!token)throw Error('Set ADMIN_TOKEN in .dev.vars');
let worker,browser;
const log=[];
const persistPath=`.wrangler/verification-${Date.now()}`;
function start(){worker=spawn(process.execPath,[fileURLToPath(cli),'dev','--no-bundle','--port','8788','--var','MOCK_MODE:true','--var','REFRESH_SECS:5','--var','WATCH_SECS:12','--persist-to',persistPath],{cwd:root,env:{...process.env,WRANGLER_SEND_METRICS:'false'},stdio:['ignore','pipe','pipe']});for(const stream of [worker.stdout,worker.stderr])stream.on('data',d=>log.push(d.toString()));}
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function ready(){for(let i=0;i<90;i++){if(worker.exitCode!==null)throw Error(`Worker exited ${worker.exitCode}: ${log.join('').slice(-3000)}`);try{if((await fetch(`${base}/api/health`,{signal:AbortSignal.timeout(1500)})).ok)return;}catch{}await delay(1000);}throw Error(`Worker startup timed out: ${log.join('').slice(-3000)}`);}
async function get(path){const r=await fetch(`${base}${path}`);assert.equal(r.status,200,path);return r.json();}
async function admin(action){const r=await fetch(`${base}/api/admin/${action}`,{method:'POST',headers:{Authorization:`Bearer ${token}`}});assert.equal(r.status,200,action);return r.json();}
async function stop(){if(!worker||worker.exitCode!==null)return;const done=new Promise(r=>worker.once('exit',r));if(process.platform==='win32')execFileSync('taskkill',['/PID',String(worker.pid),'/T','/F'],{stdio:'ignore'});else worker.kill();await done;}
try{
  start();await ready();console.log('Local workerd ready');
  const cold=await Promise.all(Array.from({length:12},()=>fetch(`${base}/api/state`)));
  for(const r of cold)assert.ok([200,503].includes(r.status));
  await delay(300);
  let state=await get('/api/state');assert.equal(state.meta.source,'mock');assert.ok(state.rank.icon);assert.ok(state.history.length);
  assert.equal(state.history.length,1,'parallel cold polls produce one initial refresh');
  let health=await get('/api/health');
  for(let i=0;i<50 && (!health.next_alarm || health.next_alarm<=Date.now() || health.refreshing);i++){await delay(100);health=await get('/api/health');}
  assert.ok(health.next_alarm>Date.now());assert.ok(health.watch_until>Date.now());
  const unauthorized=await fetch(`${base}/api/admin/refresh`,{method:'POST'});assert.equal(unauthorized.status,401);
  const rank=await fetch(`${base}/api/rank.txt`);assert.equal(rank.headers.get('cache-control'),'no-store');assert.match(await rank.text(),/RR/);
  const redirect=await fetch(`${base}/overlay/rank-card.html?scale=1.25`,{redirect:'manual'});assert.equal(redirect.status,307);assert.match(redirect.headers.get('location'),/scale=1.25/);
  for(const path of ['/overlay/rank-card','/overlay/history','/overlay/session','/overlay/alerts','/dashboard/','/fonts/anton.woff2','/fonts/rajdhani.woff2'])assert.equal((await fetch(`${base}${path}`)).status,200,path);
  const checked=state.meta.checked_at;await delay(6100);state=await get('/api/state');assert.notEqual(state.meta.checked_at,checked);console.log('Alarm refreshed state automatically');
  await admin('session/reset');state=await get('/api/state');assert.equal(state.session.net_rr,0);assert.equal(state.session.wins,0);
  browser=await chromium.launch({headless:true});const page=await browser.newPage({viewport:{width:1440,height:1150}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`${base}/dashboard/`);await page.locator('#rank').filter({hasText:'Diamond'}).waitFor();
  await page.locator('#admin-token').fill(token);await page.locator('#token-form button[type=submit]').click();
  await page.locator('#url-form [name=accent]').fill('#FF4655');await page.locator('#url-form [name=scale]').fill('1.25');assert.match(await page.locator('#source-url').inputValue(),/accent=%23FF4655/);assert.match(await page.locator('#source-size').innerText(),/650 × 200/);
  await mkdir(new URL('../artifacts',import.meta.url),{recursive:true});await page.screenshot({path:fileURLToPath(new URL('../artifacts/dashboard.png',import.meta.url)),fullPage:true});
  let alertState=await get('/api/state');
  const overlay=await browser.newPage({viewport:{width:1920,height:1080}});
  await overlay.route('**/api/state',route=>route.fulfill({json:alertState}));
  await overlay.goto(`${base}/overlay/alerts?poll=5`);await delay(700);
  for(const kind of ['win','loss','draw','rank_up','derank','new_peak']){
    alertState=await admin(`test/${kind}`);
    // This page previews one injected event at a time; background mock games
    // are independently covered by the alarm test above.
    alertState.events=[alertState.events.at(-1)];
    const titles={win:'VICTORY',loss:'DEFEAT',draw:'DRAW',rank_up:'RANK UP',derank:'RANK ADJUSTED',new_peak:'NEW PEAK'};
    await overlay.locator('.alert-title').filter({hasText:titles[kind]}).waitFor({timeout:15000});
    await overlay.locator('.alert-stage').waitFor({state:'detached',timeout:25000});
  }
  await overlay.close();
  await page.goto(`${base}/overlay/rank-card`);await page.locator('.rank-top h1').waitFor();await page.screenshot({path:fileURLToPath(new URL('../artifacts/rank-card.png',import.meta.url)),clip:{x:0,y:0,width:520,height:160}});
  const edgeState=await get('/api/state');edgeState.rank={...edgeState.rank,tier_id:27,tier_name:'Radiant',rr:712,leaderboard:32};
  await page.route('**/api/state',route=>route.fulfill({json:edgeState}));
  await page.goto(`${base}/overlay/rank-card?scale=1.25&accent=%23FF4655`);await page.locator('.elite-rr').filter({hasText:'712 RR · #32'}).waitFor();assert.equal(await page.locator('.rr-track').isVisible(),false);
  edgeState.rank.tier_id=0;edgeState.rank.tier_name='Unranked';
  await page.reload();await page.locator('.rank-top h1').filter({hasText:'Unranked'}).waitFor();assert.equal(await page.locator('.rr-section').isVisible(),false);
  await page.unroute('**/api/state');
  await page.goto(`${base}/overlay/history?count=10`);await page.locator('.match-chip').first().waitFor();assert.ok(await page.locator('.match-chip').count()<=10);
  await page.goto(`${base}/overlay/history?view=acts`);await page.locator('.act-chip').first().waitFor();
  await page.goto(`${base}/overlay/session`);await page.locator('svg.chart .line').waitFor();assert.equal(errors.length,0,errors.join('; '));
  console.log('Browser overlays, acts, session chart, alert queue, dashboard, URL builder verified');
  await browser.close();browser=null;
  state=await get('/api/state');const savedId=state.next_event_id;
  await stop();start();await ready();state=await get('/api/state');assert.ok(state.next_event_id>=savedId);console.log('SQLite state persisted across restart');
  await delay(18000);health=await get('/api/health');assert.equal(health.next_alarm,null);assert.ok(health.watch_until<Date.now());console.log('Alarm stopped after watch expiry');
  console.log('Runtime verification passed');
}finally{await browser?.close();await stop();}
