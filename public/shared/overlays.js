import { TrackerFeed, options, params, signed, escape as e, safeImage, imagePath, accent, setupOverlay } from './client.js';
setupOverlay();
const root = document.querySelector('#overlay');
const type = document.body.dataset.overlay;
const img = (url, cls = '', alt = '') => { const src = safeImage(url); return src ? `<img class="${cls}" src="${e(src)}" alt="${e(alt)}">` : ''; };
const tone = n => n > 0 ? 'positive' : n < 0 ? 'negative' : 'muted';
function rankCard(s) {
  const r = s.rank, session = s.session;
  const elite = r.tier_id >= 24 || /immortal|radiant/i.test(r.tier_name);
  root.dataset.unranked = r.tier_id === 0;
  // Preserve the bar node between updates so CSS can interpolate its width.
  if (!root.querySelector('.rank-main')) root.innerHTML = `<div class="badge-slot"></div><div class="rank-main"><div class="rank-top"><h1></h1><span class="delta number"></span></div><div class="rr-section"><div class="rr-track"><div class="rr-fill" style="width:0%"></div></div><div class="rr-label"></div><div class="elite-rr"></div></div><div class="session-line"></div></div>`;
  root.querySelector('.badge-slot').innerHTML = img(r.icon, 'rank-badge', r.tier_name);
  root.querySelector('h1').textContent = r.tier_name;
  const delta = root.querySelector('.delta'); delta.className = `delta number ${tone(r.last_change)}`; delta.textContent = `${signed(r.last_change)} ${r.last_change > 0 ? '▲' : r.last_change < 0 ? '▼' : '—'}`;
  root.querySelector('.rr-track').hidden = elite; root.querySelector('.rr-label').hidden = elite;
  root.querySelector('.rr-fill').style.width = `${Math.max(0,Math.min(100,r.rr))}%`;
  root.querySelector('.rr-label').innerHTML = `<span class="eyebrow">RANK RATING</span><span>${e(r.rr)} / 100 RR</span>`;
  root.querySelector('.elite-rr').hidden = !elite;
  root.querySelector('.elite-rr').textContent = `${r.rr} RR${r.leaderboard ? ` · #${r.leaderboard}` : ''}`;
  root.querySelector('.session-line').innerHTML = session ? `<span class="eyebrow">SESSION</span> <span class="${tone(session.net_rr)}">${signed(session.net_rr)} RR</span> <span class="muted">·</span> ${session.wins}W ${session.losses}L${session.draws ? ` ${session.draws}D` : ''}` : '<span class="muted">WAITING FOR FIRST MATCH</span>';
}
let historyKey = '';
function history(s) {
  const data = options.acts ? s.acts.slice(0,options.count) : s.history.slice(0,options.count);
  const key = JSON.stringify(data); if (key === historyKey) return; historyKey = key;
  root.classList.toggle('dense', options.count > 7);
  if (!data.length) { root.innerHTML = '<div class="waiting">NO HISTORY YET</div>'; return; }
  root.innerHTML = data.map(m => options.acts ? `<div class="act-chip panel">${img(m.icon,'',m.end_tier)}<div><div class="eyebrow">${e(m.season)}</div><div class="heading">${e(m.end_tier)}</div><p>${m.wins}W · ${m.games} GAMES</p></div></div>` : `<article class="match-chip ${e(m.result)}">${img(imagePath('maps',m.map_id,'listviewicon'),'map-image')}<div class="chip-content"><div class="chip-top"><span class="chip-map">${e(m.map)}</span><span class="chip-result ${m.result==='win'?'positive':m.result==='loss'?'negative':'muted'}">${m.result==='win'?'W':m.result==='loss'?'L':'D'}</span></div><div class="chip-top"><span class="chip-rr number ${tone(m.rr_change)}">${signed(m.rr_change)}</span><span class="chip-extra">${m.derank_protected?'⛨':''}${m.refunded_rr?` +${m.refunded_rr} refund`:''}</span></div><div class="chip-bottom">${img(imagePath('agents',m.agent_id,'displayicon'),'agent-icon',m.agent)}<span class="chip-kda number">${m.kda.join(' / ')}</span><span class="chip-extra optional">${e(m.score)}</span></div></div></article>`).join('');
}
export function chartData(s) {
  if (!s.session) return null;
  const games = s.history.filter(m => Date.parse(m.at) >= Date.parse(s.session.started_at)).reverse();
  const values = [s.session.start_elo,...games.map(m=>m.elo_after)];
  if (values.at(-1)!==s.rank.elo) values.push(s.rank.elo);
  const min = Math.min(...values)-15, max = Math.max(...values)+15;
  const x = i => 8+i*280/Math.max(1,values.length-1), y = v => 82-(v-min)*70/(max-min);
  const points = values.map((v,i)=>`${x(i)},${y(v)}`).join(' ');
  const boundaries=[]; for(let v=Math.ceil(min/100)*100;v<=max;v+=100) boundaries.push({ value:v, y:y(v) });
  return { values, points, boundaries, last:{x:x(values.length-1),y:y(values.at(-1))} };
}
function session(s) {
  if (!s.session) { root.innerHTML='<div class="waiting">SESSION STARTS WITH A MATCH</div>'; return; }
  const a=s.session, c=chartData(s);
  root.innerHTML=`<div class="session-header"><div><div class="eyebrow">SESSION RATING</div><h1 class="net-rr number ${tone(a.net_rr)}">${signed(a.net_rr)} <small>RR</small></h1></div><div class="record number"><span class="positive">${a.wins}W</span> <span class="muted">—</span> <span class="negative">${a.losses}L</span> <span class="muted">${a.draws}D</span></div></div><svg class="chart" viewBox="0 0 356 95" role="img" aria-label="Session Elo trend">${c.boundaries.map(b=>`<line class="boundary" x1="8" x2="288" y1="${b.y}" y2="${b.y}"/><text x="296" y="${b.y+3}">${b.value} ELO</text>`).join('')}<polygon class="area" points="8,89 ${c.points} ${c.last.x},89"/><polyline class="line" points="${c.points}"/><circle class="point" cx="${c.last.x}" cy="${c.last.y}" r="3"/></svg><div class="eyebrow optional">${a.wins+a.losses+a.draws} MATCHES · ${e(new Date(a.started_at).toLocaleTimeString([], {hour:'2-digit',minute:'2-digit'}))} START</div>`;
}
const queue=[]; let playing=false;
function sound(up) {
  if (!options.sound) return;
  try { const ctx=new AudioContext(); const gain=ctx.createGain(); gain.connect(ctx.destination); gain.gain.setValueAtTime(.06,ctx.currentTime); gain.gain.exponentialRampToValueAtTime(.001,ctx.currentTime+.6); const osc=ctx.createOscillator();osc.type='sine';osc.frequency.setValueAtTime(up?440:220,ctx.currentTime);osc.frequency.exponentialRampToValueAtTime(up?880:150,ctx.currentTime+.4);osc.connect(gain);osc.start();osc.stop(ctx.currentTime+.6);osc.onended=()=>ctx.close(); } catch { /* Sound is optional and browser policy may block it. */ }
}
function particles(canvas,color,duration) {
  const ctx=canvas.getContext('2d'); if(!ctx)return;
  const dots=Array.from({length:70},()=>({x:400,y:190,vx:(Math.random()-.5)*6,vy:(Math.random()-.7)*5,size:Math.random()*3+1}));
  const start=performance.now(); let last=start;
  function frame(time) { const elapsed=time-start;if(elapsed>duration)return;const dt=Math.min(3,(time-last)/16.67);last=time;ctx.clearRect(0,0,800,420);ctx.fillStyle=color;ctx.globalAlpha=Math.max(0,1-elapsed/duration);for(const p of dots){p.x+=p.vx*dt;p.y+=p.vy*dt;p.vy+=.015*dt;ctx.fillRect(p.x,p.y,p.size,p.size);}requestAnimationFrame(frame); }requestAnimationFrame(frame);
}
function nextAlert() {
  if(playing || !queue.length)return;playing=true;
  const event=queue.shift();const rank=event.kind==='rank_up'||event.kind==='new_peak';const down=event.kind==='derank';const duration=rank?6000:down?2800:3200;
  const title=event.kind==='rank_up'?'RANK UP':event.kind==='new_peak'?'NEW PEAK':down?'RANK ADJUSTED':event.result==='win'?'VICTORY':event.result==='loss'?'DEFEAT':'DRAW';
  const stage=document.createElement('div');stage.className=`alert-stage playing ${down?'derank':''}`;stage.style.setProperty('--duration',`${duration}ms`);stage.style.setProperty('--accent',/^#[\da-f]{6}$/i.test(event.accent)?event.accent:'#FF4655');
  stage.innerHTML=`<div class="glow"></div>${rank?'<canvas width="800" height="420"></canvas>':''}<div class="alert-content">${rank||down?img(event.icon,'alert-badge'):''}<div class="eyebrow">COMPETITIVE</div><h1 class="alert-title">${title}</h1>${event.kind==='match_result'?`<div class="alert-delta number ${tone(event.rr_change)}">${signed(event.rr_change)} <span style="font-size:30px">RR</span></div>`:''}<p class="alert-caption">${e(event.to ?? (event.result==='win'?'KEEP THE MOMENTUM':event.result==='loss'?'NEXT ROUND, NEXT CHANCE':'EVENLY MATCHED'))}</p></div>`;
  root.replaceChildren(stage);sound(rank||event.result==='win');if(rank)particles(stage.querySelector('canvas'),event.accent,duration-500);
  setTimeout(()=>{stage.remove();playing=false;nextAlert();},duration);
}
const feed=new TrackerFeed();
feed.addEventListener('update',({detail:s})=>{accent(s);if(type==='rank-card')rankCard(s);else if(type==='history')history(s);else if(type==='session')session(s);});
feed.addEventListener('event',({detail:event})=>{if(type==='alerts' && event.kind!=='session_start' && !event.quiet){queue.push(event);if(queue.length>20)queue.shift();nextAlert();}});
feed.start();
