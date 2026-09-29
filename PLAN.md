# Valorant Rank Live Tracker — Build Plan

OBS overlays that show **ISHQ#tejo2**'s (region `ap`, platform `pc`) current rank, RR, last-game RR change,
session stats and match history. Written in **Rust**, compiled to WebAssembly, deployed on **Cloudflare Workers**
(Free plan) with one **Durable Object**, and runnable locally with `wrangler dev`, using the same code in both places.

> **Why not Fermyon Cloud (the previous target)?** Its free Starter plan allows 100,000 requests/month, has no background
> jobs, and caps handlers at 30s, which forced lazy refresh and client polling. Its future is also unclear since Akamai
> bought Fermyon (Dec 2025). The Workers Free plan allows 100,000 requests/**day**, serves static files for free, and Durable
> Object alarms give us background refresh. `crates/core` does no I/O, so it moves over unchanged; only the glue crate is rewritten.

---

## 1. Decisions

| Area | Choice | Why |
|---|---|---|
| Hosting | Cloudflare Workers **Free** plan (no credit card) | 100k Worker requests/day, free static assets, Durable Objects included |
| Backend language | Rust → `wasm32-unknown-unknown` via **`worker` 0.8.x** (workers-rs) + `worker-build` | Keeps Rust; target already installed (rustc 1.93.1) |
| Tooling | **Wrangler 4** (npm devDependency; Node 24 installed) | `wrangler dev` runs the real runtime (workerd) locally; `wrangler deploy` ships it |
| Data source | HenrikDev unofficial API (v3 MMR, v2 MMR history, v4 match) | Only practical source of MMR/RR history without a Riot production key |
| Game assets | valorant-api.com (tier icons/colors, map & agent images) | Free, no key, stable CDN URLs |
| State + logic | **One Durable Object** `Tracker` (SQLite backend, key-value API) | One instance for the whole app, so refreshes never race and no lock key is needed. 30s CPU per request **even on Free** |
| Freshness | **Alarm-driven refresh while someone is watching** | DO alarms refresh in the background every 60s and stop 10 min after the last poll, so there are no upstream calls when OBS is closed |
| Overlay updates | Client polling of `/api/state` (default 30s); WebSocket push is a later upgrade (Phase 8) | Polling is simple and robust; push via WebSocket Hibernation is possible here, unlike on Fermyon |
| Frontend | Plain HTML/CSS/JS served as **Workers Static Assets**, no build step | Static asset requests are free and unlimited (don't count toward 100k/day) |
| Code structure | Cargo workspace: `core` (pure logic) + `api` (Worker + DO glue) | Logic is unit-testable on the host with `cargo test`, no Wasm needed |

### Where code runs (important for the Free plan's CPU limits)
- **Worker** (`#[event(fetch)]`): capped at **10ms CPU** per request on Free. It only forwards `/api/*` to the Durable Object.
- **Durable Object** `Tracker`: **30s CPU** per request/alarm on Free (each request resets the budget). All JSON parsing,
  upstream fetches and session math run here, so large v4 match responses aren't a problem.
- Static files never touch either one; Cloudflare serves them before the Worker runs.

### What Cloudflare changes compared with the Fermyon plan
- **Background refresh exists.** A DO alarm refreshes every `REFRESH_SECS` while anyone polled in the last `WATCH_SECS`.
  Poll responses come straight from stored state, so they're fast and never wait on HenrikDev.
- **No lock, fewer races.** There is one `Tracker` instance, so refreshes can't overlap (an in-memory `refreshing` flag covers
  requests that arrive while a refresh is awaiting a fetch).
- **Quotas (Free, reset daily at 00:00 UTC):** Workers 100k requests/day; DO 100k requests/day, 13,000 GB-s/day,
  100k rows written/day, 5M rows read/day, 5 GB storage. Hitting a limit makes requests fail until the reset; there's no bill.
  Section 10 covers the budget.
- **Local ≠ cloud storage.** `wrangler dev` keeps its own DO state in `.wrangler/state/`; the deployed app has its own.
- **Still no local Riot client access.** The cloud can't read your PC's lockfile, so the "instant RR via local client" idea stays
  dropped. Expect RR to show up about 1–3 min after a match ends; HenrikDev's own latency is most of that.

---

## 2. Architecture

```
                         Cloudflare Workers  (or `wrangler dev` on your PC)
                      ┌─────────────────────────────────────────────────────┐
  OBS Browser         │  Static Assets  (public/)       free, not a request  │
  Sources  ───────────┼─►  /overlay/*  /shared/*.js|css  /fonts/*            │
    │                 │                                                     │
    │  GET every 30s  │  Worker  (Rust, 10ms CPU)                           │
    └─────────────────┼─►  /api/*  ──► forward to DO "tracker"              │
                      │                   │                                 │
  Dashboard ──────────┼─►                 ▼                                 │
  (admin, Bearer)     │  Durable Object  Tracker  (Rust, 30s CPU, SQLite)   │
  Nightbot !rank ─────┼─►  fetch():  return stored state, extend watch,     │
                      │              make sure an alarm is set              │
                      │  alarm():    refresh() ─────────────────────────────┼──► api.henrikdev.xyz
                      │              ├─ diff newest match_id                │
                      │              ├─ fetch MMR + match details if new    │
                      │              ├─ core::apply_refresh() → events      │──► valorant-api.com
                      │              ├─ save state                          │     (tier table, 1×/day)
                      │              └─ re-arm in 60s while watched         │
                      └─────────────────────────────────────────────────────┘
  Images (icons/maps/agents) load directly in OBS from media.valorant-api.com
```

---

## 3. Upstream APIs (checked against docs.henrikdev.xyz, v4.5.0)

Base: `https://api.henrikdev.xyz` · Header: `Authorization: <HENRIK_API_KEY>` · Basic key ≈ 30 req/min.

| Purpose | Endpoint | Fields used |
|---|---|---|
| Resolve PUUID (once) + current rank | `GET /valorant/v3/mmr/ap/pc/ISHQ/tejo2` | `data.account.puuid` |
| Current rank (by PUUID afterwards) | `GET /valorant/v3/by-puuid/mmr/ap/pc/{puuid}` | `data.current.tier.{id,name}`, `.rr`, `.last_change`, `.elo`, `.leaderboard_placement`; `data.peak.tier.name`, `data.peak.season.short`; `data.seasonal[].{season.short,end_tier,wins,games}` |
| RR history (cheap change detector) | `GET /valorant/v2/by-puuid/mmr-history/ap/pc/{puuid}` | `history[].{match_id,tier,map,season,rr,last_change,elo,refunded_rr,was_derank_protected,date}` |
| Match details (per new match) | `GET /valorant/v4/match/ap/{match_id}` | teams won/rounds, own player's agent + K/D/A, score |
| Tier table | `GET https://valorant-api.com/v1/competitivetiers` (last entry = current episode) | `tiers[].{tier,tierName,largeIcon,smallIcon,color,backgroundColor}` |
| Map/agent images | `https://media.valorant-api.com/maps/{mapUuid}/listviewicon.png`, `.../agents/{agentUuid}/displayicon.png` | built directly from ids, no lookup |

> ⚠ The snippet from your research mixes **v2 field names** (`current_data.currenttierpatched`, `ranking_in_tier`,
> `images`, `peak_data`) with the **v3 URL**. Those fields don't exist in v3; use the v3 fields above.

Outbound calls use `worker::Fetch`. Workers have no outbound allowlist, and one refresh makes at most ~6 subrequests (limit: 50).

---

## 4. Project layout

```
valorant tracker/
├─ PLAN.md
├─ wrangler.toml
├─ package.json               devDependency: wrangler@4 (pinned, run via `npx wrangler …`)
├─ Cargo.toml                 workspace: crates/core, crates/api
├─ .dev.vars                  HENRIK_API_KEY=… / ADMIN_TOKEN=…   (local secrets, gitignored)
├─ .gitignore                 target/, node_modules/, .dev.vars, .wrangler/, crates/api/build/
├─ crates/
│  ├─ core/                   pure Rust, no Cloudflare deps, fully unit-tested   (unchanged from the Fermyon plan)
│  │  ├─ src/henrik.rs        serde models for v3 mmr, v2 mmr-history, v4 match
│  │  ├─ src/model.rs         TrackerState, RankView, MatchEntry, Session, Event
│  │  ├─ src/normalize.rs     raw API → RankView / MatchEntry
│  │  ├─ src/tracker.rs       apply_refresh(): diff, session math, event generation
│  │  └─ src/tiers.rs         tier table → name / icon / accent color
│  │  └─ tests/fixtures/*.json   real responses captured in Phase 0
│  └─ api/                    Worker + Durable Object (cdylib, `worker` crate)
│     ├─ src/lib.rs           #[event(fetch)]: forward /api/* to the Tracker DO
│     ├─ src/tracker_do.rs    #[durable_object] Tracker: routes, alarm loop, (Phase 8) WebSockets
│     ├─ src/config.rs        env.var / env.secret → Config
│     ├─ src/upstream.rs      worker::Fetch, Authorization header, 429 backoff
│     ├─ src/store.rs         typed get/put over DO storage
│     ├─ src/auth.rs          Bearer ADMIN_TOKEN check (constant-time compare, `subtle`)
│     ├─ src/mock.rs          MOCK mode: replays fixtures + scripted sequence
│     └─ mock/*.json          trimmed fixtures, compiled in with include_str! (Workers have no filesystem)
└─ public/                    served as Workers Static Assets
   ├─ shared/client.js        polling feed, event detection, OBS status hooks
   ├─ shared/theme.css        design tokens, panel shapes, animations
   ├─ fonts/                  self-hosted Anton + Rajdhani (woff2)
   ├─ overlay/rank-card.html
   ├─ overlay/history.html
   ├─ overlay/session.html
   ├─ overlay/alerts.html
   └─ dashboard/index.html
```

Rules:
- **`core` never does I/O or reads the clock.** `api` fetches data, passes `now` in, and stores the result.
  This lets every edge case be tested with `cargo test -p core` on Windows.
- `core` must also build for `wasm32-unknown-unknown`: serde/serde_json only; if `chrono` is used, turn off its
  default features (no `clock`), since `now` is always passed in.

---

## 5. `wrangler.toml` (target shape)

```toml
name = "valorant-rank-tracker"
main = "crates/api/build/index.js"          # generated by worker-build
compatibility_date = "2026-09-01"

[build]
cwd = "crates/api"
command = "cargo install -q worker-build && worker-build --release"
watch_dir = ["crates"]

[assets]
directory = "./public"                      # served before the Worker; /api/* has no file, so it falls through

[[durable_objects.bindings]]
name = "TRACKER"
class_name = "Tracker"

[[migrations]]
tag = "v1"
new_sqlite_classes = ["Tracker"]            # Free plan only supports SQLite-backed DOs

[vars]
RIOT_NAME = "ISHQ"
RIOT_TAG = "tejo2"
REGION = "ap"
PLATFORM = "pc"
REFRESH_SECS = "60"
WATCH_SECS = "600"                          # keep refreshing this long after the last poll
SESSION_GAP_HOURS = "6"
MOCK_MODE = "false"

# Secrets (never in this file):
#   local:  .dev.vars  →  HENRIK_API_KEY=…  ADMIN_TOKEN=…   (MOCK_MODE=true can go here too)
#   cloud:  npx wrangler secret put HENRIK_API_KEY  /  npx wrangler secret put ADMIN_TOKEN
```

Shape of the Rust side (a sketch, not final code):

```rust
// crates/api/src/lib.rs — the Worker: forward only, stays far under 10ms CPU
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let stub = env.durable_object("TRACKER")?.id_from_name("tracker")?.get_stub()?;
    stub.fetch_with_request(req).await
}

// crates/api/src/tracker_do.rs — all the real work
#[durable_object]
pub struct Tracker { state: State, env: Env, mem: RefCell<Mem> }   // Mem: cached state, refreshing flag

impl DurableObject for Tracker {
    fn new(state: State, env: Env) -> Self { /* … */ }
    async fn fetch(&self, req: Request) -> Result<Response> { /* route §8, extend watch, ensure alarm */ }
    async fn alarm(&self) -> Result<Response> { /* refresh, then re-arm while watched (§7) */ }
}
```

⚠ `storage.set_alarm(x)`: an `i64` or `Duration` is an **offset from now**, and a `chrono::DateTime<Utc>` is an absolute time.
`get_alarm()` returns absolute epoch ms. Use `Duration::from_secs(…)` everywhere to avoid mixing them up.

---

## 6. Data model (Durable Object storage)

The DO's SQLite storage is used through its key-value API (`storage().get/put`). Each `put` is one row written.

| Key | Content |
|---|---|
| `state` | the whole `TrackerState` below (~20 KB; max value size 2 MB) |
| `puuid` | resolved PUUID (so a Riot name change doesn't break anything) |
| `watch_until` | epoch ms; polling extends it, and the alarm loop stops after it (written at most 1×/min) |
| `backoff` | `{ until, step }` set after a 429/5xx |
| `tiers` | tier table + `fetched_at` (refreshed daily) |

The Fermyon plan's `tracker:lock` key is gone. The single DO instance plus an in-memory `refreshing` flag replaces it.

```jsonc
{
  "rank":    { "tier_id": 17, "tier_name": "Diamond 2", "rr": 67, "elo": 1467, "last_change": 21,
               "icon": "https://media.valorant-api.com/competitivetiers/…/17/largeicon.png",
               "accent": "#b489c4", "leaderboard": null },
  "peak":    { "tier_name": "Diamond 3", "season": "e9a3" },
  "acts":    [ { "season": "e9a2", "end_tier": "Platinum 3", "wins": 31, "games": 58 } ],
  "history": [ { "match_id": "…", "map": "Ascent", "map_id": "…", "agent": "Jett", "agent_id": "…",
                 "result": "win", "score": "13-9", "kda": [21,14,6], "rr_change": 21,
                 "rr_after": 67, "elo_after": 1467, "derank_protected": false, "refunded_rr": 0,
                 "at": "2026-09-29T15:04:00Z" } ],          // newest first, capped at 50
  "session": { "started_at": "…", "start_elo": 1410, "net_rr": 57, "wins": 4, "losses": 1, "draws": 0 },
  "events":  [ { "id": 42, "kind": "rank_up", "at": "…", "from": "Diamond 1", "to": "Diamond 2", "quiet": false } ],  // last 20
  "next_event_id": 43,
  "meta":    { "checked_at": "…", "changed_at": "…", "stale": false, "source": "henrikdev" }
}
```

Event kinds: `match_result` (win/loss/draw with RR change), `rank_up`, `derank`, `new_peak`, `session_start`.

**`quiet` events:** a refresh is a *catch-up* when the previous `checked_at` is older than `WATCH_SECS` (nobody was watching,
e.g. games played with OBS closed). Its events are stored with `quiet: true`. Overlays still update rank and history, but
`alerts` doesn't play them, so opening OBS never replays old games on stream, whatever order the overlays load in.

---

## 7. Refresh algorithm

### On any poll: `GET /api/state` (also `/api/rank.txt`) → DO `fetch()`
1. Load `state` (from memory, else storage). If there is **no state at all** (first run ever), refresh inline (step 1 below) before answering.
2. Extend the watch window: `watch_until = now + WATCH_SECS` (write to storage only if the stored value is > 60s behind).
3. If no alarm is pending (`get_alarm()` is `None`), set one:
   in `max(0, checked_at + REFRESH_SECS − now)`, or at `backoff.until` if backing off.
4. Return the state with `meta.stale = now − checked_at > 3 × REFRESH_SECS` and `Cache-Control: no-store`.

`/api/rank.txt` refreshes inline when older than `REFRESH_SECS`, because Nightbot calls it rarely and usually while offline.

### `alarm()` → `refresh()`
1. If `refreshing` is set (a refresh is already running), return. Set `refreshing = true`.
2. Resolve the PUUID, using the stored one if present, else calling v3 MMR by name.
3. Fetch v2 MMR history. If the newest `match_id` equals the newest stored one → update `checked_at`, save, go to step 7.
4. For new matches (oldest first, max 3 per refresh): fetch v4 match → build `MatchEntry`
   (result from teams' `won` flags, not from the RR sign, so draws, derank protection and refunds are handled). Then fetch v3 MMR once.
5. `core::tracker::apply_refresh(prev, input, now, cfg)` → new state + events (with `catch_up = prev.checked_at older than WATCH_SECS`):
   - Session: if there's no session, or the first new game started > `SESSION_GAP_HOURS` after the last one, start a new session
     with `start_elo = first_new_game.elo_after − first_new_game.rr_change`.
   - `net_rr = current_elo − start_elo`. Using elo keeps this correct across rank-ups and deranks.
   - `rank_up` / `derank` when `tier_id` changes; `new_peak` when the tier passes the stored peak.
   - `catch_up` → every new event gets `quiet: true`.
6. Save `state`. On 429/5xx: set `backoff` (60s → 120s → 300s), keep the old state, re-arm at `backoff.until`, and return.
7. Clear `refreshing`. **Re-arm only while watched:** if `now < watch_until` → `set_alarm(Duration::from_secs(REFRESH_SECS))`;
   otherwise set no alarm, and the loop sleeps until the next poll.

Return `Ok` from `alarm()` even after an upstream error (backoff already handles it). A returned `Err` makes Cloudflare retry the alarm
automatically, which would bypass our backoff.

Processing is keyed on `match_id`, so running a refresh twice gives the same result.

---

## 8. HTTP API

| Method | Route | Auth | Purpose |
|---|---|---|---|
| GET | `/api/state` | public | Full state from storage; keeps the refresh loop alive |
| GET | `/api/health` | public | Version, last check, next alarm, `watch_until`, backoff status |
| GET | `/api/rank.txt` | public | `Diamond 2 · 67 RR · +21 last game` for a Nightbot/StreamElements `!rank` command |
| POST | `/api/admin/refresh` | Bearer | Ignore freshness and refresh now (inline) |
| POST | `/api/admin/session/reset` | Bearer | Start a new session from the current elo |
| POST | `/api/admin/test/{win\|loss\|draw\|rank_up\|derank}` | Bearer | Inject a fake event to preview alerts |
| GET | `/api/ws` | public | *(Phase 8)* WebSocket; the DO pushes the state after every change |

---

## 9. Overlays & design

**Style:** Valorant dark theme. Glass panels (`rgba(15,25,35,.78)` + backdrop blur), angled corner cut via `clip-path`,
`#FF4655` red, `#ECE8E1` off-white, **Anton** headings and **Rajdhani** numbers (self-hosted).
The accent color **follows the current tier's color** from valorant-api.com (purple at Diamond, green at Ascendant, and so on).
Transparent backgrounds.

| Overlay | Size | Content |
|---|---|---|
| `rank-card` | 520×160 | Tier badge, rank name, animated RR bar 0–100, last-game `+21 ▲` / `−16 ▼`, session line |
| `history` | 900×110 | Last 5–10 games as chips: map image, W/L/D color, RR change, agent icon, K/D/A. `?view=acts` shows past act badges instead |
| `session` | 400×220 | Net RR, W–L–D, SVG line of elo across the session with tier boundaries marked |
| `alerts` | 1920×1080 | RR pop after each game; rank-up reveal (tier-colored glow, particles, badge scale-in, ~6s); short, low-key derank |

```
┌────────────────────────────────────────────────┐
│ ◆◆   DIAMOND 2                        +21 ▲    │
│ ◆◆   █████████████████░░░░░░░   67 / 100 RR    │
│      SESSION  +57 RR   ·   4W  1L              │
└────────────────────────────────────────────────┘
```

**URLs have no `.html`:** Static Assets answer `/overlay/rank-card.html` with a 307 redirect to `/overlay/rank-card`
(default `html_handling`), so use the short form in OBS: `…/overlay/rank-card?scale=1.25`.

URL parameters (all overlays): `?scale=1.25&accent=tier|#FF4655&poll=30&count=5&layout=compact&sound=0`.

`shared/client.js`:
- polls `/api/state`, fires `update` on any change and `event` for event ids newer than the last seen one
  (on first load, last seen = max id, so a page reload never replays old alerts; `quiet` events never fire as alerts);
- if `window.obsstudio` exists, polls every **30s while streaming/recording** and **every 5 min otherwise**
  (browser source → Page permissions → "Read access to OBS status information"). The 5-min idle poll is shorter than
  `WATCH_SECS` (10 min), so the refresh loop keeps running while OBS is open;
- stops polling while the page is hidden; on network errors it keeps the last data and never shows errors on stream.

Edge cases in the UI:
- **Immortal/Radiant:** no 100 RR cap, so show the RR number + leaderboard `#` instead of the bar.
- **Unranked** (tier 0): show "UNRANKED", no bar.
- **Derank-protected loss:** show a small shield icon.
- **RR refund:** show a `+X refund` chip.

---

## 10. Request budget (Workers Free, per day, resets 00:00 UTC)

| Item | Heavy day: 10h streamed + 14h OBS open idle | Free limit/day | Used |
|---|---|---|---|
| Worker requests (4 overlays: 480/h live, 48/h idle) | ~4,800 + ~670 ≈ **5,500** | 100,000 | ~6% |
| DO requests (1 per `/api/*` request + alarms) | ~5,500 + ~1,440 ≈ **7,000** | 100,000 | ~7% |
| DO rows written (`state` per refresh + `watch_until` ≤1/min) | ≤ **2,900** | 100,000 | ~3% |
| DO duration (billed at 128 MB while active) | even 24h active = 10,800 GB-s | 13,000 GB-s | worst case 83% |
| Static files (page loads, fonts) | any | unlimited | free |

**Upstream:** at most 1 refresh per `REFRESH_SECS` (~60 history calls/h + ~2 per game) no matter how many overlays poll,
and none once the watch window expires. That's far below HenrikDev's 30/min.
**If a limit is ever hit:** requests fail until 00:00 UTC and overlays keep showing their last data. Raise `poll`, or switch
to WebSocket push (Phase 8), which drops polling traffic to about zero. Workers Paid ($5/mo) is the next step up if ever needed.

---

## 11. Security

- `HENRIK_API_KEY` and `ADMIN_TOKEN` are **Worker secrets** (`wrangler secret put`), and `.dev.vars` locally.
  They're never sent to the browser or committed (`.dev.vars` is gitignored).
- Admin routes require `Authorization: Bearer <ADMIN_TOKEN>`. The dashboard stores the token in localStorage on your PC only.
- The public `/api/state` can't burn the HenrikDev quota: the refresh cadence is fixed by the alarm, not by request volume.
- A flood of requests could use up the 100k/day Worker quota (overlays would freeze on the last data until 00:00 UTC).
  The overlay URLs only live in your OBS and Nightbot. If abuse ever happens, move to a custom domain on Cloudflare and add a WAF rate-limit rule.
- Only your own account is shown.

---

## 12. Build phases

### Phase 0: Scaffold & capture real data
- [ ] `npm init -y` → `npm i -D wrangler@4`
- [ ] Cargo workspace (`crates/core`, `crates/api`). Take `crates/api`'s `Cargo.toml` from the workers-rs template
      (`cargo generate cloudflare/workers-rs`) so `worker` 0.8.x and the release profile (LTO, `opt-level`) are right
- [ ] `wrangler.toml` from §5; a hello-world Worker that forwards to a DO that answers `/api/health`
- [ ] `public/` with a placeholder page; `.dev.vars` with `HENRIK_API_KEY` and `ADMIN_TOKEN`; `.gitignore`
- [ ] Capture fixtures with curl for ISHQ#tejo2: v3 MMR, v2 MMR history, one v4 match, competitivetiers
- [ ] Confirm the open items in §14 against the fixtures and a first `wrangler dev` run

**Done when:** `npx wrangler dev` serves `/api/health` and a static page on `http://localhost:8787`, and the fixtures are
saved in `crates/core/tests/fixtures/`.

### Phase 1: `core` crate (logic + tests)
- [ ] serde models (lenient: `#[serde(default)]`, `Option<…>` on anything that might be missing)
- [ ] `normalize` → `RankView`, `MatchEntry`; `tiers` → icon URL + accent color
- [ ] `apply_refresh()`: new-match diff, session start/gap/reset, net RR by elo, W/L/D, events, `quiet` on catch-up
- [ ] Tests: rank-up across tiers, derank, draw, derank-protected loss, RR refund, Immortal RR > 100,
      unranked, 6h session gap, duplicate refresh (idempotent), > 3 new matches at once, catch-up → quiet events

**Done when:** `cargo test -p core` passes and `cargo build -p core --target wasm32-unknown-unknown` compiles.

### Phase 2: `api` Worker + Durable Object
- [ ] Config from vars/secrets; typed store over DO storage; upstream client with `Authorization` header + backoff
- [ ] Poll handler + alarm loop + watch window (§7); PUUID resolve and cache; tier table cache (daily)
- [ ] Routes from §8; admin Bearer auth; `MOCK_MODE=true` replays trimmed fixtures and a scripted win → loss → rank-up sequence

**Done when:** with `wrangler dev`, `curl http://localhost:8787/api/state` shows ISHQ#tejo2's real rank, `/api/health` shows
the alarm re-arming every 60s and stopping ~10 min after the last poll, and mock mode cycles events.

### Phase 3: Rank Card overlay + OBS (**MVP**)
- [ ] `theme.css` tokens, fonts, panel shape; `client.js` polling feed
- [ ] `rank-card.html` with RR bar animation, delta chip and tier accent
- [ ] Add to OBS as a Browser Source from `http://localhost:8787/overlay/rank-card`

**Done when:** the card updates by itself after a real comp game while `wrangler dev` is running.

### Phase 4: Deploy to Cloudflare
- [ ] `npx wrangler login` (free Cloudflare account, no card); pick a `workers.dev` subdomain
- [ ] `npx wrangler deploy` → `npx wrangler secret put HENRIK_API_KEY` → `npx wrangler secret put ADMIN_TOKEN`
- [ ] Switch OBS URLs to `https://valorant-rank-tracker.<subdomain>.workers.dev/overlay/…`
- [ ] After one stream, check Workers & Pages → Metrics and the Durable Objects metrics against §10; `npx wrangler tail` for logs

**Done when:** the overlay works from the workers.dev URL with `wrangler dev` stopped.

### Phase 5: History + Session overlays
- [ ] `history.html` (match chips, slide-in animation, `?view=acts`)
- [ ] `session.html` (net RR, W–L–D, SVG elo line)

### Phase 6: Alerts
- [ ] RR pop, rank-up reveal (canvas particles), derank, new peak; optional sound; skip `quiet` events

**Done when:** each `/api/admin/test/*` event plays cleanly in OBS.

### Phase 7: Dashboard
- [ ] Status (last check, next alarm, watch window, stale/backoff, current state), force refresh, reset session, test-event buttons
- [ ] Live previews of each overlay + URL builder for the query parameters

### Phase 8: Hardening & extras
- [ ] **WebSocket push (optional):** `/api/ws` → DO `state.accept_web_socket()` (Hibernation API, so idle sockets cost nothing);
      after each refresh that changes state, send it to `state.get_websockets()`. "Watched" becomes
      `open sockets > 0 || now < watch_until`. `client.js` tries WS first and falls back to polling
- [ ] `/api/rank.txt` + a Nightbot/StreamElements `!rank` command
- [ ] README: setup, OBS settings, deploy, troubleshooting

**Done when:** a 4-hour stream runs with no stale data on screen and stays within the budget in §10.

---

## 13. OBS setup (each overlay)
Sources → **+** → Browser → URL as above (no `.html`), width/height from §9.
- Leave **"Shutdown source when not visible" unchecked**.
- Keep the default custom CSS (transparent background).
- Page permissions → **"Read access to OBS status information"** (for streaming-aware polling).

Use one source per overlay so each can be positioned and scaled in OBS on its own.

---

## 14. Open items to confirm in Phase 0
- [ ] Exact nesting and entry count of v2 MMR history (`data.history[]`?)
- [ ] v4 match-by-id path and the team/player field names
- [ ] HenrikDev map/agent ids = valorant-api.com UUIDs (so image URLs can be built directly)
- [ ] Format of the tier `color` field in valorant-api.com (8-char RGBA hex?)
- [ ] `worker-build` inside a Cargo workspace with `[build] cwd = "crates/api"` (fallback: make the api crate the repo root)
- [ ] `storage().get/put` (key-value API) works on a SQLite-backed DO in `worker` 0.8.x (fallback: `storage().sql()` with one table)
- [ ] Alarms fire under local `wrangler dev` and state persists in `.wrangler/state/` between runs
- [ ] The 307 redirect for `/…/rank-card.html` keeps the query string (we avoid it anyway by using extensionless URLs)

## 15. Out of scope (for now)
- Real-time RR from the local Riot client (not possible from the cloud; unofficial API)
- Showing other players' ranks
- Multi-account support (config is per deployment; a second Worker with different vars would work)
