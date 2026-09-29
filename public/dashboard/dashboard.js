import { TrackerFeed, signed, safeImage, accent } from '/shared/client.js';
const $ = id => document.getElementById(id);
const sizes = { 'rank-card':[520,160], history:[900,110], session:[400,220], alerts:[1920,1080] };
const feed = new TrackerFeed();
const formatTime = ms => ms ? new Date(ms).toLocaleTimeString() : 'Sleeping';
const regions = { ap:'ASIA PACIFIC', eu:'EUROPE', na:'NORTH AMERICA', kr:'KOREA', latam:'LATIN AMERICA', br:'BRAZIL' };
function showAccount(h) {
  if(!h.account)return;const i=h.account.lastIndexOf('#');
  $('account-name').firstChild.textContent=i<0?h.account:h.account.slice(0,i);$('account-tag').textContent=i<0?'':h.account.slice(i);
  $('account-region').textContent=[regions[h.region]||h.region?.toUpperCase(),h.platform?.toUpperCase()].filter(Boolean).join(' · ');
}
feed.addEventListener('update', ({detail:s}) => {
  accent(s); $('rank').textContent = s.rank.tier_name; $('rating').textContent = `${s.rank.rr} RR · ${signed(s.rank.last_change)} last game`;
  const icon = safeImage(s.rank.icon); $('tier-icon').hidden = !icon; if(icon)$('tier-icon').src=icon;
  $('peak').textContent = s.peak ? `PEAK / ${s.peak.tier_name}` : '';
  $('session-summary').textContent = s.session ? `${signed(s.session.net_rr)} RR · ${s.session.wins}W / ${s.session.losses}L / ${s.session.draws}D` : 'Session starts with a match';
  $('raw-state').textContent=JSON.stringify(s,null,2);
});
feed.start();
async function health() {
  try {
    const r=await fetch('/api/health',{cache:'no-store'});if(!r.ok)throw Error();const h=await r.json();showAccount(h);
    $('mode').textContent=h.mode==='mock'?'MOCK PREVIEW':`LIVE / ${(h.region||'').toUpperCase()}`;$('checked').textContent=h.checked_at?formatTime(Date.parse(h.checked_at)):'No data';$('alarm').textContent=formatTime(h.next_alarm);$('watch').textContent=h.watch_until>Date.now()?formatTime(h.watch_until):'Expired';$('backoff').textContent=h.backoff.until>Date.now()?`${Math.ceil((h.backoff.until-Date.now())/1000)}s · HTTP ${h.backoff.status??'network'}`:'None';
    $('connection').textContent=h.stale?'WAITING / STALE':'TRACKER ONLINE';$('status-dot').className=`status-dot ${h.stale?'stale':'good'}`;
    $('health-note').textContent=!h.configured?'Configure the HenrikDev key in local secrets.':h.mode==='mock'?'Scripted preview data. Set MOCK_MODE=false for live rank.':`Background refresh every ${h.refresh_secs}s while watched.`;
  } catch { $('connection').textContent='CONNECTION LOST';$('status-dot').className='status-dot stale'; }
}
health();setInterval(()=>{if(!document.hidden)health();},15000);
const form=$('url-form');
function buildURL() {
  const data=new FormData(form);const overlay=data.get('overlay');const url=new URL(`/overlay/${overlay}`,location.origin);
  for(const [k,v] of data)if(k!=='overlay')url.searchParams.set(k,v);
  $('source-url').value=url.href;$('open-url').href=url.href;
  const [w,h]=sizes[overlay];const scale=Number(data.get('scale'))||1;
  $('source-size').textContent=`OBS Browser Source: ${Math.ceil(w*scale)} × ${Math.ceil(h*scale)} px. Keep “Shutdown source when not visible” unchecked.`;
}
form.addEventListener('input',buildURL);form.addEventListener('change',buildURL);form.addEventListener('submit',e=>e.preventDefault());buildURL();
for(const button of document.querySelectorAll('[data-source]'))button.addEventListener('click',()=>{form.elements.overlay.value=button.dataset.source;buildURL();$('builder').scrollIntoView({behavior:'smooth',block:'center'});});
$('copy-url').addEventListener('click',async()=>{try{await navigator.clipboard.writeText($('source-url').value);$('copy-url').textContent='Copied';}catch{$('source-url').select();$('copy-url').textContent='Select & copy';}setTimeout(()=>$('copy-url').textContent='Copy URL',1800);});
try{$('admin-token').value=localStorage.getItem('tracker-admin-token')||'';}catch{}
$('token-form').addEventListener('submit',e=>{e.preventDefault();try{localStorage.setItem('tracker-admin-token',$('admin-token').value.trim());$('action-status').textContent='Token saved on this device.';}catch{$('action-status').textContent='Browser storage unavailable; token remains usable for this page.';}});
$('forget-token').addEventListener('click',()=>{try{localStorage.removeItem('tracker-admin-token');}catch{}$('admin-token').value='';$('action-status').textContent='Token removed from this device.';});
for(const button of document.querySelectorAll('[data-action]'))button.addEventListener('click',async()=>{
  const token=$('admin-token').value.trim();if(!token){$('action-status').textContent='Enter your admin token first.';return;}
  button.disabled=true;$('action-status').textContent='Working…';
  try{const r=await fetch(`/api/admin/${button.dataset.action}`,{method:'POST',headers:{Authorization:`Bearer ${token}`}});const data=await r.json();if(!r.ok)throw new Error(data.error||(data.backoff?.until>Date.now()?'Upstream is backing off; retry after the countdown.':`Request failed (${r.status})`));if(data.rank)feed.ingest(data);else if(data.state)feed.ingest(data.state);$('action-status').textContent=button.dataset.action.startsWith('test/')?'Preview event queued. Watch the alerts preview.':button.dataset.action==='session/reset'?'Session reset to the current Elo.':'Tracker refreshed.';await health();}
  catch(e){$('action-status').textContent=e.message||'Unable to reach tracker.';}finally{button.disabled=false;}
});
