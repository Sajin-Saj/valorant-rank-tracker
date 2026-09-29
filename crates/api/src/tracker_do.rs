use crate::{
    auth,
    config::Config,
    mock,
    store::{self, Backoff, TierCache},
    upstream,
};
use std::{cell::RefCell, time::Duration};
use tracker_core::{
    model::*,
    normalize,
    tracker::{self, RefreshInput},
};
use worker::*;

#[derive(Default)]
struct Mem {
    loaded: bool,
    state: Option<TrackerState>,
    watch_until: i64,
    persisted_watch: i64,
    backoff: Backoff,
    tiers: TierCache,
    refreshing: bool,
    mock_step: u64,
}

#[durable_object]
pub struct Tracker {
    state: State,
    env: Env,
    mem: RefCell<Mem>,
}

fn now() -> i64 {
    Date::now().as_millis() as i64
}
fn json<T: serde::Serialize>(value: &T) -> Result<Response> {
    let mut response = Response::from_json(value)?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}

impl Tracker {
    async fn load(&self) -> Result<()> {
        if self.mem.borrow().loaded {
            return Ok(());
        }
        let state = store::get::<TrackerState>(&self.state, "state").await?;
        let watch = store::get::<i64>(&self.state, "watch_until")
            .await?
            .unwrap_or(0);
        let backoff = store::get::<Backoff>(&self.state, "backoff")
            .await?
            .unwrap_or_default();
        let tiers = store::get::<TierCache>(&self.state, "tiers")
            .await?
            .unwrap_or_default();
        let mock_step = store::get::<u64>(&self.state, "mock_step")
            .await?
            .unwrap_or(0);
        let mut mem = self.mem.borrow_mut();
        if !mem.loaded {
            mem.loaded = true;
            mem.state = state;
            mem.watch_until = watch;
            mem.persisted_watch = watch;
            mem.backoff = backoff;
            mem.tiers = tiers;
            mem.mock_step = mock_step;
        }
        Ok(())
    }
    async fn watch(&self, cfg: &Config, time: i64) -> Result<()> {
        let until = time + cfg.watch_secs as i64 * 1000;
        let persist = {
            let mut mem = self.mem.borrow_mut();
            mem.watch_until = until;
            until - mem.persisted_watch >= (cfg.watch_secs / 2).clamp(1, 60) as i64 * 1000
        };
        if persist {
            store::put(&self.state, "watch_until", &until).await?;
            self.mem.borrow_mut().persisted_watch = until;
        }
        Ok(())
    }
    async fn schedule(&self, cfg: &Config, time: i64) -> Result<()> {
        if time >= self.mem.borrow().watch_until {
            self.state.storage().delete_alarm().await?;
            return Ok(());
        }
        if self.state.storage().get_alarm().await?.is_some() {
            return Ok(());
        }
        let due = {
            let mem = self.mem.borrow();
            let checked = mem
                .state
                .as_ref()
                .and_then(|s| tracker::epoch(&s.meta.checked_at))
                .unwrap_or(0);
            let due = if mem.refreshing {
                time + cfg.refresh_secs as i64 * 1000
            } else {
                checked + cfg.refresh_secs as i64 * 1000
            };
            due.max(mem.backoff.until)
        };
        self.state
            .storage()
            .set_alarm(Duration::from_millis((due - time).max(1) as u64))
            .await
    }
    async fn save(&self, value: TrackerState) -> Result<()> {
        store::put(&self.state, "state", &value).await?;
        self.mem.borrow_mut().state = Some(value);
        Ok(())
    }
    async fn tier_table(
        &self,
        cfg: &Config,
        time: i64,
    ) -> std::result::Result<tracker_core::tiers::TierTable, upstream::Failure> {
        let cache = self.mem.borrow().tiers.clone();
        if !cache.table.tiers.is_empty() && time - cache.fetched_at < 86_400_000 {
            return Ok(cache.table);
        }
        let table = if cfg.mock {
            serde_json::from_str::<
                tracker_core::henrik::Envelope<Vec<tracker_core::tiers::TierTable>>,
            >(include_str!("../../core/tests/fixtures/tiers.json"))
            .map_err(|_| upstream::Failure {
                status: None,
                retry_secs: None,
            })?
            .data
            .pop()
            .unwrap_or_default()
        } else {
            match upstream::tiers().await {
                Ok(table) => table,
                Err(_) if !cache.table.tiers.is_empty() => return Ok(cache.table),
                Err(err) => return Err(err),
            }
        };
        let cache = TierCache {
            table: table.clone(),
            fetched_at: time,
        };
        store::put(&self.state, "tiers", &cache).await?;
        self.mem.borrow_mut().tiers = cache;
        Ok(table)
    }
    async fn refresh_inner(
        &self,
        cfg: &Config,
        time: i64,
    ) -> std::result::Result<TrackerState, upstream::Failure> {
        let prev = self.mem.borrow().state.clone().unwrap_or_default();
        if !cfg.mock && cfg.key.is_empty() {
            return Err(upstream::Failure {
                status: Some(503),
                retry_secs: None,
            });
        }
        let tiers = self.tier_table(cfg, time).await?;
        if cfg.mock {
            let step = self.mem.borrow().mock_step;
            let result = mock::refresh(&prev, step, time, cfg.core(), &tiers);
            store::put(&self.state, "mock_step", &(step + 1)).await?;
            self.mem.borrow_mut().mock_step = step + 1;
            return Ok(result);
        }
        let stored_puuid = store::get::<String>(&self.state, "puuid").await?;
        let initial = if stored_puuid.is_none() {
            Some(upstream::mmr(cfg, None).await?)
        } else {
            None
        };
        let puuid = stored_puuid
            .or_else(|| initial.as_ref().map(|m| m.account.puuid.clone()))
            .unwrap_or_default();
        if initial.is_some() {
            store::put(&self.state, "puuid", &puuid).await?;
        }
        let history = upstream::history(cfg, &puuid).await?.history;
        let pending = tracker::pending_matches(&history, &prev);
        if pending.is_empty() && !prev.meta.checked_at.is_empty() {
            let mut next = prev;
            next.meta.checked_at = tracker::timestamp(time);
            next.meta.stale = false;
            next.meta.catching_up = false;
            return Ok(next);
        }
        let mut entries = vec![];
        for h in pending {
            let game = upstream::game(cfg, &h.match_id).await?;
            entries.push(normalize::match_entry(h, &game, &puuid).map_err(|_| {
                upstream::Failure {
                    status: Some(502),
                    retry_secs: None,
                }
            })?);
        }
        let mmr = match initial {
            Some(m) => m,
            None => upstream::mmr(cfg, Some(&puuid)).await?,
        };
        let mut next = tracker::apply_refresh(
            &prev,
            RefreshInput {
                rank: normalize::rank(&mmr, &tiers),
                peak: normalize::peak(&mmr),
                acts: normalize::acts(&mmr, &tiers),
                matches: entries,
                source: "henrikdev".into(),
            },
            time,
            cfg.core(),
        );
        next.meta.catching_up = !tracker::pending_matches(&history, &next).is_empty()
            && (prev.meta.catching_up
                || tracker::epoch(&prev.meta.checked_at)
                    .is_none_or(|t| time - t > cfg.watch_secs as i64 * 1000));
        Ok(next)
    }
    async fn refresh(&self, cfg: &Config, time: i64) -> Result<bool> {
        {
            let mut mem = self.mem.borrow_mut();
            if mem.refreshing || time < mem.backoff.until {
                return Ok(false);
            }
            mem.refreshing = true;
        }
        let result = self.refresh_inner(cfg, time).await;
        let saved = match result {
            Ok(value) => {
                let result = self.save(value).await;
                match result {
                    Ok(()) => {
                        let empty = Backoff::default();
                        let needs_clear = self.mem.borrow().backoff.step != 0;
                        let persisted = if needs_clear {
                            store::put(&self.state, "backoff", &empty).await
                        } else {
                            Ok(())
                        };
                        self.mem.borrow_mut().backoff = empty;
                        persisted.map(|_| true)
                    }
                    Err(e) => Err(e),
                }
            }
            Err(err) => {
                let backoff = self
                    .mem
                    .borrow()
                    .backoff
                    .next(now(), err.status, err.retry_secs);
                self.mem.borrow_mut().backoff = backoff.clone();
                store::put(&self.state, "backoff", &backoff)
                    .await
                    .map(|_| false)
            }
        };
        self.mem.borrow_mut().refreshing = false;
        saved
    }
    fn view(&self, cfg: &Config, time: i64) -> Option<TrackerState> {
        self.mem.borrow().state.clone().map(|mut s| {
            s.meta.stale = tracker::epoch(&s.meta.checked_at)
                .is_none_or(|t| time - t > cfg.refresh_secs as i64 * 3000);
            s
        })
    }
    async fn route(&self, req: Request) -> Result<Response> {
        let cfg = Config::from_env(&self.env);
        let path = req.path();
        let method = req.method();
        if path.starts_with("/api/admin/") && !auth::authorized(&req, &cfg.admin) {
            let mut response = json(&serde_json::json!({"error":"Unauthorized"}))?.with_status(401);
            response.headers_mut().set("WWW-Authenticate", "Bearer")?;
            return Ok(response);
        }
        self.load().await?;
        let time = now();
        match (method, path.as_str()) {
            (Method::Get, "/api/health") => {
                let alarm = self.state.storage().get_alarm().await?;
                let state = self.view(&cfg, time);
                let mem = self.mem.borrow();
                json(
                    &serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"account":format!("{}#{}",cfg.name,cfg.tag),"region":cfg.region,"platform":cfg.platform,"mode":if cfg.mock {"mock"}else{"live"},"configured":cfg.mock || !cfg.key.is_empty(),"admin_configured":!cfg.admin.is_empty(),"checked_at":state.as_ref().map(|s|&s.meta.checked_at),"stale":state.as_ref().is_none_or(|s|s.meta.stale),"next_alarm":alarm,"watch_until":mem.watch_until,"backoff":mem.backoff,"refreshing":mem.refreshing,"refresh_secs":cfg.refresh_secs,"watch_secs":cfg.watch_secs}),
                )
            }
            (Method::Get, "/api/state" | "/api/rank.txt") => {
                self.watch(&cfg, time).await?;
                let missing = self.mem.borrow().state.is_none();
                let old = self
                    .mem
                    .borrow()
                    .state
                    .as_ref()
                    .and_then(|s| tracker::epoch(&s.meta.checked_at))
                    .is_none_or(|t| time - t >= cfg.refresh_secs as i64 * 1000);
                if missing || (path == "/api/rank.txt" && old) {
                    self.refresh(&cfg, time).await?;
                }
                self.schedule(&cfg, now()).await?;
                let Some(state) = self.view(&cfg, now()) else {
                    return Ok(json(&serde_json::json!({"error":"No data yet. Configure HENRIK_API_KEY or enable MOCK_MODE; check health for retry status."}))?.with_status(503));
                };
                if path == "/api/rank.txt" {
                    let mut response = Response::ok(format!(
                        "{} · {} RR · {:+} last game",
                        state.rank.tier_name, state.rank.rr, state.rank.last_change
                    ))?;
                    response.headers_mut().set("Cache-Control", "no-store")?;
                    return Ok(response);
                }
                json(&state)
            }
            (Method::Post, "/api/admin/refresh") => {
                if self.mem.borrow().refreshing {
                    return Ok(json(
                        &serde_json::json!({"error":"Refresh in progress; retry shortly"}),
                    )?
                    .with_status(409));
                }
                self.watch(&cfg, time).await?;
                let refreshed = self.refresh(&cfg, time).await?;
                self.schedule(&cfg, now()).await?;
                Ok(json(&serde_json::json!({"refreshed":refreshed,"state":self.view(&cfg,now()),"backoff":self.mem.borrow().backoff}))?.with_status(if refreshed {200}else{503}))
            }
            (Method::Post, "/api/admin/session/reset") => {
                if self.mem.borrow().refreshing {
                    return Ok(json(
                        &serde_json::json!({"error":"Refresh in progress; retry shortly"}),
                    )?
                    .with_status(409));
                }
                let Some(mut state) = self.view(&cfg, time) else {
                    return Ok(
                        json(&serde_json::json!({"error":"Load state first"}))?.with_status(409)
                    );
                };
                tracker::reset_session(&mut state, time);
                self.save(state.clone()).await?;
                json(&state)
            }
            (Method::Post, _) if path.starts_with("/api/admin/test/") => {
                if self.mem.borrow().refreshing {
                    return Ok(json(
                        &serde_json::json!({"error":"Refresh in progress; retry shortly"}),
                    )?
                    .with_status(409));
                }
                let Some(mut state) = self.view(&cfg, time) else {
                    return Ok(
                        json(&serde_json::json!({"error":"Load state first"}))?.with_status(409)
                    );
                };
                let test = path.trim_start_matches("/api/admin/test/");
                let (kind, result, change) = match test {
                    "win" => (EventKind::MatchResult, Some(Outcome::Win), Some(21)),
                    "loss" => (EventKind::MatchResult, Some(Outcome::Loss), Some(-16)),
                    "draw" => (EventKind::MatchResult, Some(Outcome::Draw), Some(0)),
                    "rank_up" => (EventKind::RankUp, None, None),
                    "derank" => (EventKind::Derank, None, None),
                    "new_peak" => (EventKind::NewPeak, None, None),
                    _ => {
                        return Ok(json(&serde_json::json!({"error":"Unknown test event"}))?
                            .with_status(404))
                    }
                };
                let mut e = tracker::event(kind, &tracker::timestamp(time), false, &state.rank);
                e.result = result;
                e.rr_change = change;
                e.from = Some("Diamond 1".into());
                e.to = Some(state.rank.tier_name.clone());
                tracker::push_event(&mut state, e);
                state.meta.changed_at = tracker::timestamp(time);
                self.save(state.clone()).await?;
                json(&state)
            }
            _ => Ok(json(&serde_json::json!({"error":"Not found"}))?.with_status(404)),
        }
    }
}

impl DurableObject for Tracker {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            mem: RefCell::new(Mem::default()),
        }
    }
    async fn fetch(&self, req: Request) -> Result<Response> {
        self.route(req).await
    }
    async fn alarm(&self) -> Result<Response> {
        let result = async {
            self.load().await?;
            let cfg = Config::from_env(&self.env);
            if now() < self.mem.borrow().watch_until {
                self.refresh(&cfg, now()).await?;
            }
            self.schedule(&cfg, now()).await
        }
        .await;
        if result.is_err() {
            console_error!("Tracker alarm storage/scheduling failure");
        }
        // Upstream failures have persisted backoff; do not trigger runtime retries.
        Response::ok("ok")
    }
}
