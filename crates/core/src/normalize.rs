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
    })
}
