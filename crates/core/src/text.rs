//! One-line chat replies for Nightbot/StreamElements `!commands` (served at `/api/text/{kind}`).
use crate::model::*;

pub const KINDS: [&str; 7] = [
    "rank", "session", "lastgame", "peak", "winrate", "streak", "stats",
];

fn signed(n: i32) -> String {
    format!("{n:+}")
}
fn outcome(o: &Outcome) -> &'static str {
    match o {
        Outcome::Win => "Win",
        Outcome::Loss => "Loss",
        Outcome::Draw => "Draw",
    }
}
fn record(games: &[&MatchEntry]) -> (usize, usize, usize) {
    let count = |o: Outcome| games.iter().filter(|m| m.result == o).count();
    (
        count(Outcome::Win),
        count(Outcome::Loss),
        count(Outcome::Draw),
    )
}

/// The reply for a command, or `None` for an unknown kind.
pub fn reply(state: &TrackerState, kind: &str) -> Option<String> {
    let r = &state.rank;
    Some(match kind {
        "rank" => {
            let board = r
                .leaderboard
                .map(|n| format!(" · #{n}"))
                .unwrap_or_default();
            format!(
                "{} · {} RR{board} · {} last game",
                r.tier_name,
                r.rr,
                signed(r.last_change)
            )
        }
        "session" => match &state.session {
            Some(s) => format!(
                "Session: {} RR · {}W {}L {}D",
                signed(s.net_rr),
                s.wins,
                s.losses,
                s.draws
            ),
            None => "No games this session yet".into(),
        },
        "lastgame" => match state.history.first() {
            Some(m) => {
                let mut parts = vec![
                    format!(
                        "Last game: {} {} on {} as {}",
                        outcome(&m.result),
                        m.score,
                        m.map,
                        m.agent
                    ),
                    format!("{}/{}/{}", m.kda[0], m.kda[1], m.kda[2]),
                ];
                if let Some(p) = &m.perf {
                    parts.push(format!("{} ACS", p.acs));
                    parts.push(format!("{}% HS", p.hs_pct));
                }
                parts.push(format!("{} RR", signed(m.rr_change)));
                match m.perf.as_ref().and_then(|p| p.mvp.as_ref()) {
                    Some(Mvp::Match) => parts.push("Match MVP".into()),
                    Some(Mvp::Team) => parts.push("Team MVP".into()),
                    None => {}
                }
                parts.join(" · ")
            }
            None => "No games tracked yet".into(),
        },
        "peak" => match &state.peak {
            Some(p) if !p.season.is_empty() => format!("Peak: {} ({})", p.tier_name, p.season),
            Some(p) => format!("Peak: {}", p.tier_name),
            None => "No peak recorded yet".into(),
        },
        "winrate" => {
            let games: Vec<_> = state.history.iter().take(20).collect();
            if games.is_empty() {
                return Some("No games tracked yet".into());
            }
            let (w, l, d) = record(&games);
            format!(
                "Last {}: {w}W {l}L {d}D · {}% win rate",
                games.len(),
                (w as f64 * 100.0 / games.len() as f64).round()
            )
        }
        "streak" => match &state.streak {
            Some(s) if s.result == Outcome::Win => format!("On a {}-game win streak 🔥", s.count),
            Some(s) => format!("On a {}-game loss streak", s.count),
            None => "No active streak".into(),
        },
        "stats" => {
            let games: Vec<_> = state
                .history
                .iter()
                .filter_map(|m| m.perf.as_ref().map(|p| (m, p)))
                .take(10)
                .collect();
            if games.is_empty() {
                return Some("No match stats yet".into());
            }
            let n = games.len() as f64;
            let avg = |f: fn(&Perf) -> u32| {
                (games.iter().map(|(_, p)| f(p) as f64).sum::<f64>() / n).round()
            };
            let (kills, deaths) = games
                .iter()
                .fold((0, 0), |(k, d), (m, _)| (k + m.kda[0], d + m.kda[1]));
            format!(
                "Last {} avg: {} ACS · {} ADR · {}% HS · {:.2} K/D",
                games.len(),
                avg(|p| p.acs),
                avg(|p| p.adr),
                avg(|p| p.hs_pct),
                kills as f64 / deaths.max(1) as f64
            )
        }
        _ => return None,
    })
}
