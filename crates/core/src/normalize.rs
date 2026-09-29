use crate::{henrik, model::*, tiers::TierTable};

pub fn rank(mmr: &henrik::Mmr, tiers: &TierTable) -> RankView {
    let c = &mmr.current;
    RankView {
        tier_id: c.tier.id,
        tier_name: if c.tier.id == 0 {
            "Unranked".into()
        } else {
            c.tier.name.clone()
        },
        rr: c.rr,
        elo: c.elo,
        last_change: c.last_change,
        icon: tiers.icon(c.tier.id),
        accent: tiers.color(c.tier.id),
        leaderboard: c.leaderboard_placement,
    }
}
pub fn peak(mmr: &henrik::Mmr) -> Option<PeakView> {
    mmr.peak.as_ref().map(|p| PeakView {
        tier_id: p.tier.id,
        tier_name: p.tier.name.clone(),
        season: p.season.short.clone(),
    })
}
pub fn acts(mmr: &henrik::Mmr, tiers: &TierTable) -> Vec<ActView> {
    mmr.seasonal
        .iter()
        .map(|a| ActView {
            season: a.season.short.clone(),
            end_tier: a.end_tier.name.clone(),
            tier_id: a.end_tier.id,
            icon: tiers.icon(a.end_tier.id),
            wins: a.wins,
            games: a.games,
        })
        .collect()
}

pub fn match_entry(
    h: &henrik::HistoryEntry,
    m: &henrik::Match,
    puuid: &str,
) -> Result<MatchEntry, String> {
    let p = m
        .players
        .iter()
        .find(|p| p.puuid == puuid)
        .ok_or("Account absent from match")?;
    let own = m
        .teams
        .iter()
        .find(|t| t.team_id.eq_ignore_ascii_case(&p.team_id))
        .ok_or("Player team absent")?;
    let other = m
        .teams
        .iter()
        .find(|t| !t.team_id.eq_ignore_ascii_case(&p.team_id))
        .ok_or("Opponent team absent")?;
    let result = if own.won {
        Outcome::Win
    } else if other.won {
        Outcome::Loss
    } else {
        Outcome::Draw
    };
    if !m.metadata.match_id.is_empty() && m.metadata.match_id != h.match_id {
        return Err("Match ID mismatch".into());
    }
    let map = if m.metadata.map.id.is_empty() {
        &h.map
    } else {
        &m.metadata.map
    };
    Ok(MatchEntry {
        match_id: h.match_id.clone(),
        map: map.name.clone(),
        map_id: map.id.clone(),
        agent: p.agent.name.clone(),
        agent_id: p.agent.id.clone(),
        result,
        score: format!("{}-{}", own.rounds.won, other.rounds.won),
        kda: [p.stats.kills, p.stats.deaths, p.stats.assists],
        rr_change: h.last_change,
        rr_after: h.rr,
        elo_after: h.elo,
        tier_id: h.tier.id,
        derank_protected: h.was_derank_protected,
        refunded_rr: h.refunded_rr,
        at: if m.metadata.started_at.is_empty() {
            h.date.clone()
        } else {
            m.metadata.started_at.clone()
        },
        perf: perf(m, puuid),
    })
}

fn per(total: u32, rounds: u32) -> u32 {
    (total as f64 / rounds as f64).round() as u32
}

/// ACS, ADR, HS%, first bloods and MVP for `puuid`; `None` when the match has no rounds.
pub fn perf(m: &henrik::Match, puuid: &str) -> Option<Perf> {
    let p = m.players.iter().find(|p| p.puuid == puuid)?;
    let own = m
        .teams
        .iter()
        .find(|t| t.team_id.eq_ignore_ascii_case(&p.team_id))?;
    let rounds = own.rounds.won + own.rounds.lost;
    if rounds == 0 {
        return None;
    }
    let s = &p.stats;
    let shots = s.headshots + s.bodyshots + s.legshots;
    let mut first_kills: std::collections::BTreeMap<u32, &henrik::Kill> = Default::default();
    for k in &m.kills {
        let slot = first_kills.entry(k.round).or_insert(k);
        if k.time_in_round_in_ms < slot.time_in_round_in_ms {
            *slot = k;
        }
    }
    let top = |team: Option<&str>| {
        m.players
            .iter()
            .filter(|o| team.is_none_or(|t| o.team_id.eq_ignore_ascii_case(t)))
            .all(|o| o.puuid == puuid || o.stats.score <= s.score)
    };
    Some(Perf {
        acs: per(s.score, rounds),
        adr: per(s.damage.dealt, rounds),
        hs_pct: if shots == 0 {
            0
        } else {
            (s.headshots as f64 * 100.0 / shots as f64).round() as u32
        },
        first_bloods: first_kills
            .values()
            .filter(|k| k.killer.puuid == puuid)
            .count() as u32,
        mvp: if m.players.len() > 1 && top(None) {
            Some(Mvp::Match)
        } else if m.players.len() > 1 && top(Some(&p.team_id)) {
            Some(Mvp::Team)
        } else {
            None
        },
    })
}
