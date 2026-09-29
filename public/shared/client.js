export const params = new URLSearchParams(location.search);
const bounded = (name, fallback, min, max) => { const n = Number(params.get(name) ?? fallback); return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback; };
export const options = { scale: bounded('scale', 1, .25, 3), poll: bounded('poll', 30, 5, 300), count: Math.round(bounded('count', 5, 5, 10)), compact: params.get('layout') === 'compact', sound: params.get('sound') === '1', acts: params.get('view') === 'acts', accent: params.get('accent') || 'tier' };
export const signed = n => `${n > 0 ? '+' : n < 0 ? '−' : ''}${Math.abs(n ?? 0)}`;
export const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
export function safeImage(url) { try { const u = new URL(url); return u.protocol === 'https:' && u.hostname === 'media.valorant-api.com' ? u.href : ''; } catch { return ''; } }
export function imagePath(type, id, filename) { return /^[a-f\d-]{36}$/i.test(id ?? '') ? `https://media.valorant-api.com/${type}/${id}/${filename}.png` : ''; }
export function accent(state) {
  const color = options.accent === 'tier' ? state.rank.accent : options.accent;
  document.documentElement.style.setProperty('--accent', /^#[\da-f]{6}$/i.test(color) ? color : '#FF4655');
}

export class TrackerFeed extends EventTarget {
  constructor({ fetcher = (...args) => fetch(...args) } = {}) { super(); this.fetcher = fetcher; this.lastId = null; this.lastState = ''; this.timer = null; this.active = true; this.busy = false; this.live = !window.obsstudio; this.onVisibility = () => { clearTimeout(this.timer); if (!document.hidden) this.poll(); }; }
  ingest(state) {
    if (!state?.rank || !Array.isArray(state.events)) return;
    const events = [...state.events].sort((a,b) => a.id - b.id);
    const max = Math.max(0, ...events.map(e => e.id));
    const first = this.lastId === null;
    if (first || max < this.lastId) this.lastId = max;
    const encoded = JSON.stringify(state);
    if (encoded !== this.lastState) { this.lastState = encoded; this.state = state; this.dispatchEvent(new CustomEvent('update', { detail: state })); }
    if (!first) for (const event of events) if (event.id > this.lastId) { if (!event.quiet) this.dispatchEvent(new CustomEvent('event', { detail: event })); }
    this.lastId = max;
  }
  interval() { return window.obsstudio && !this.live ? 300000 : options.poll * 1000; }
  async poll() {
    if (!this.active || document.hidden || this.busy) return;
    clearTimeout(this.timer); this.busy = true;
    try {
      const response = await this.fetcher('/api/state', { cache: 'no-store', signal: AbortSignal.timeout(15000) });
      if (response.ok && this.active && !document.hidden) this.ingest(await response.json());
    } catch { /* Keep the last good state on stream. */ }
    finally { this.busy = false; if (this.active && !document.hidden) this.timer = setTimeout(() => this.poll(), this.interval()); }
  }
  start() {
    document.addEventListener('visibilitychange', this.onVisibility);
    const status = s => { this.live = Boolean(s.streaming || s.recording); clearTimeout(this.timer); this.poll(); };
    if (window.obsstudio?.getStatus) window.obsstudio.getStatus(status);
    for (const event of ['obsStreamingStarted', 'obsStreamingStopped', 'obsRecordingStarted', 'obsRecordingStopped']) {
      window.addEventListener(event, () => window.obsstudio?.getStatus?.(status));
    }
    this.poll(); return this;
  }
  stop() { this.active = false; clearTimeout(this.timer); document.removeEventListener('visibilitychange', this.onVisibility); }
}

export function setupOverlay() {
  document.documentElement.style.setProperty('--scale', options.scale);
  document.body.classList.toggle('compact', options.compact);
  document.addEventListener('error', e => { if (e.target instanceof HTMLImageElement) e.target.style.visibility = 'hidden'; }, true);
}
