use crate::{henrik::HistoryEntry, model::*};
use chrono::{DateTime, SecondsFormat, Utc};
use std::collections::HashSet;

pub fn timestamp(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or_default()
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}
pub fn epoch(at: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|d| d.timestamp_millis())
}

#[derive(Clone, Copy)]
pub struct Config {
    pub session_gap_ms: i64,
    pub watch_ms: i64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            session_gap_ms: 6 * 3_600_000,
            watch_ms: 600_000,
        }
    }
}
pub struct RefreshInput {
    pub rank: RankView,
    pub peak: Option<PeakView>,
    pub acts: Vec<ActView>,
    pub matches: Vec<MatchEntry>,
    pub source: String,
}

// Fetch the oldest unprocessed games first; never advance past an unfetched game.
pub fn pending_matches<'a>(
    history: &'a [HistoryEntry],
    state: &TrackerState,
) -> Vec<&'a HistoryEntry> {
    let known: HashSet<&str> = state.history.iter().map(|m| m.match_id.as_str()).collect();
    let mut seen = HashSet::new();
    let mut pending: Vec<_> = history
        .iter()
        .take_while(|h| !known.contains(h.match_id.as_str()))
        .filter(|h| !h.match_id.is_empty() && seen.insert(h.match_id.as_str()))
        .collect();
    pending.sort_by_key(|h| epoch(&h.date).unwrap_or(0));
    pending.truncate(3);
    pending
}

pub fn push_event(state: &mut TrackerState, mut event: Event) {
    event.id = state.next_event_id;
    state.next_event_id += 1;
    state.events.push(event);
    if state.events.len() > 20 {
        state.events.drain(..state.events.len() - 20);
    }
}
pub fn event(kind: EventKind, at: &str, quiet: bool, rank: &RankView) -> Event {
    Event {
        id: 0,
        kind,
        at: at.into(),
        quiet,
        from: None,
        to: None,
        result: None,
        rr_change: None,
        match_id: None,
        icon: rank.icon.clone(),
        accent: rank.accent.clone(),
    }
}

pub fn reset_session(state: &mut TrackerState, now: i64) {
    let at = timestamp(now);
    state.session = Some(Session {
        started_at: at.clone(),
        start_elo: state.rank.elo,
        ..Session::default()
    });
    push_event(
        state,
        event(EventKind::SessionStart, &at, false, &state.rank),
    );
    state.meta.changed_at = at;
}

pub fn apply_refresh(
    prev: &TrackerState,
    input: RefreshInput,
    now: i64,
    cfg: Config,
) -> TrackerState {
    let mut state = prev.clone();
    let at = timestamp(now);
    let quiet = prev.meta.catching_up
        || epoch(&prev.meta.checked_at).is_none_or(|last| now - last > cfg.watch_ms);
    let known: HashSet<_> = prev.history.iter().map(|m| m.match_id.clone()).collect();
    let mut seen = HashSet::new();
    let mut matches: Vec<_> = input
        .matches
        .into_iter()
        .filter(|m| {
            !m.match_id.is_empty()
                && !known.contains(&m.match_id)
                && seen.insert(m.match_id.clone())
        })
        .collect();
    matches.sort_by_key(|m| epoch(&m.at).unwrap_or(0));
    let mut last_at = prev.history.first().and_then(|m| epoch(&m.at));
    for m in matches {
        let game_at = epoch(&m.at).unwrap_or(now);
        let reset = state.session.is_none()
            || last_at.is_some_and(|last| game_at - last > cfg.session_gap_ms);
        if reset {
            state.session = Some(Session {
                started_at: m.at.clone(),
                start_elo: m.elo_after - m.rr_change,
                ..Session::default()
            });
            push_event(
                &mut state,
                event(EventKind::SessionStart, &m.at, quiet, &input.rank),
            );
        }
        if let Some(session) = state.session.as_mut() {
            if game_at >= epoch(&session.started_at).unwrap_or(0) {
                match m.result {
                    Outcome::Win => session.wins += 1,
                    Outcome::Loss => session.losses += 1,
                    Outcome::Draw => session.draws += 1,
                }
            }
        }
        let mut e = event(EventKind::MatchResult, &m.at, quiet, &input.rank);
        e.result = Some(m.result.clone());
        e.rr_change = Some(m.rr_change);
        e.match_id = Some(m.match_id.clone());
        push_event(&mut state, e);
        last_at = Some(game_at);
        state.history.insert(0, m);
    }
    state.history.truncate(50);
    if let Some(session) = state.session.as_mut() {
        session.net_rr = input.rank.elo - session.start_elo;
    }
    if prev.rank.tier_id != input.rank.tier_id && !prev.meta.checked_at.is_empty() {
        let kind = if input.rank.tier_id > prev.rank.tier_id {
            EventKind::RankUp
        } else {
            EventKind::Derank
        };
        let mut e = event(kind, &at, quiet, &input.rank);
        e.from = Some(prev.rank.tier_name.clone());
        e.to = Some(input.rank.tier_name.clone());
        push_event(&mut state, e);
    }
    let old_peak = prev.peak.as_ref().map(|p| p.tier_id).unwrap_or(0);
    let mut peak = input.peak.clone().or_else(|| prev.peak.clone());
    if prev
        .peak
        .as_ref()
        .is_some_and(|p| peak.as_ref().is_none_or(|next| p.tier_id > next.tier_id))
    {
        peak = prev.peak.clone();
    }
    if peak.as_ref().is_none_or(|p| input.rank.tier_id > p.tier_id) {
        peak = Some(PeakView {
            tier_id: input.rank.tier_id,
            tier_name: input.rank.tier_name.clone(),
            season: input
                .acts
                .first()
                .map(|a| a.season.clone())
                .unwrap_or_default(),
        });
    }
    if peak.as_ref().is_some_and(|p| p.tier_id > old_peak) && !prev.meta.checked_at.is_empty() {
        let mut e = event(EventKind::NewPeak, &at, quiet, &input.rank);
        e.to = peak.as_ref().map(|p| p.tier_name.clone());
        push_event(&mut state, e);
    }
    state.rank = input.rank;
    state.peak = peak;
    state.acts = input.acts;
    if state.rank != prev.rank
        || state.peak != prev.peak
        || state.acts != prev.acts
        || state.history != prev.history
        || state.session != prev.session
        || state.events != prev.events
    {
        state.meta.changed_at = at.clone();
    }
    state.meta.checked_at = at;
    state.meta.stale = false;
    state.meta.source = input.source;
    state
}
