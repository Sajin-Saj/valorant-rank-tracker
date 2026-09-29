use tracker_core::{
    henrik::*,
    model::*,
    normalize,
    tiers::{self, TierTable},
    tracker::{self, RefreshInput},
};
const NOW: i64 = 1_790_676_000_000;
fn rank(id: u32, elo: i32) -> RankView {
    RankView {
        tier_id: id,
        tier_name: format!("Tier {id}"),
        elo,
        rr: elo % 100,
        ..RankView::default()
    }
}
fn game(id: &str, elo: i32, delta: i32, result: Outcome, at: i64) -> MatchEntry {
    MatchEntry {
        match_id: id.into(),
        at: tracker::timestamp(at),
        elo_after: elo,
        rr_change: delta,
        result,
        ..MatchEntry::default()
    }
}
fn input(rank: RankView, matches: Vec<MatchEntry>) -> RefreshInput {
    RefreshInput {
        rank,
        matches,
        peak: None,
        acts: vec![],
        source: "test".into(),
    }
}
fn apply(prev: &TrackerState, input: RefreshInput, now: i64) -> TrackerState {
    tracker::apply_refresh(prev, input, now, tracker::Config::default())
}
fn baseline() -> TrackerState {
    apply(
        &TrackerState::default(),
        input(
            rank(18, 1590),
            vec![game("a", 1590, 20, Outcome::Win, NOW - 60_000)],
        ),
        NOW - 60_000,
    )
}

#[test]
fn rank_up_and_net_across_tiers() {
    let s = apply(
        &baseline(),
        input(rank(19, 1611), vec![game("b", 1611, 21, Outcome::Win, NOW)]),
        NOW,
    );
    assert_eq!(s.session.as_ref().unwrap().net_rr, 41);
    assert_eq!(s.session.as_ref().unwrap().wins, 2);
    assert!(s
        .events
        .iter()
        .any(|e| e.kind == EventKind::RankUp && !e.quiet));
    assert!(s.events.iter().any(|e| e.kind == EventKind::NewPeak));
}
#[test]
fn derank_draw_protection_and_refund() {
    let mut m = game("b", 1580, -10, Outcome::Loss, NOW);
    m.derank_protected = true;
    m.refunded_rr = 7;
    let s = apply(&baseline(), input(rank(17, 1580), vec![m]), NOW);
    assert!(s.events.iter().any(|e| e.kind == EventKind::Derank));
    assert!(s.history[0].derank_protected);
    assert_eq!(s.history[0].refunded_rr, 7);
    let s = apply(
        &s,
        input(
            rank(17, 1580),
            vec![game("c", 1580, 0, Outcome::Draw, NOW + 1000)],
        ),
        NOW + 1000,
    );
    assert_eq!(s.session.unwrap().draws, 1);
}
#[test]
fn idle_catch_up_is_quiet_and_idempotent() {
    let prev = baseline();
    let time = NOW + 700_000;
    let s = apply(
        &prev,
        input(
            rank(19, 1611),
            vec![game("b", 1611, 21, Outcome::Win, time)],
        ),
        time,
    );
    assert!(s
        .events
        .iter()
        .filter(|e| e.id >= prev.next_event_id)
        .all(|e| e.quiet));
    let twice = apply(&s, input(s.rank.clone(), vec![s.history[0].clone()]), time);
    assert_eq!(twice, s);
}
#[test]
fn session_gap_and_manual_reset() {
    let time = NOW + 6 * 3_600_000;
    let mut s = apply(
        &baseline(),
        input(
            rank(18, 1610),
            vec![game("b", 1610, 20, Outcome::Win, time)],
        ),
        time,
    );
    assert_eq!(s.session.as_ref().unwrap().wins, 1);
    assert_eq!(s.session.as_ref().unwrap().net_rr, 20);
    tracker::reset_session(&mut s, time + 1000);
    let s = apply(
        &s,
        input(
            rank(18, 1620),
            vec![game("c", 1620, 10, Outcome::Win, time + 2000)],
        ),
        time + 2000,
    );
    assert_eq!(s.session.unwrap().net_rr, 10);
}
#[test]
fn gap_inside_batch_starts_last_session() {
    let s = apply(
        &TrackerState::default(),
        input(
            rank(19, 1700),
            vec![
                game("a", 1600, 20, Outcome::Win, NOW - 7 * 3_600_000),
                game("b", 1700, 30, Outcome::Loss, NOW),
            ],
        ),
        NOW,
    );
    let session = s.session.unwrap();
    assert_eq!(session.wins, 0);
    assert_eq!(session.losses, 1);
    assert_eq!(session.net_rr, 30);
}
#[test]
fn batches_never_skip_games() {
    let hs: Vec<_> = (1..=8)
        .rev()
        .map(|i| HistoryEntry {
            match_id: format!("m{i}"),
            date: tracker::timestamp(NOW + i * 1000),
            ..HistoryEntry::default()
        })
        .collect();
    let mut s = TrackerState::default();
    for expected in [3, 3, 2] {
        let pending = tracker::pending_matches(&hs, &s);
        assert_eq!(pending.len(), expected);
        let entries = pending
            .iter()
            .map(|h| {
                game(
                    &h.match_id,
                    1600,
                    0,
                    Outcome::Draw,
                    tracker::epoch(&h.date).unwrap(),
                )
            })
            .collect();
        s = apply(&s, input(rank(18, 1600), entries), NOW);
    }
    assert_eq!(s.history.len(), 8);
    assert_eq!(s.history[0].match_id, "m8");
}
#[test]
fn immortal_and_unranked_and_tier_color() {
    let mut mmr = serde_json::from_str::<Envelope<Mmr>>(include_str!("fixtures/mmr.json"))
        .unwrap()
        .data;
    mmr.current.tier = Tier {
        id: 27,
        name: "Radiant".into(),
    };
    mmr.current.rr = 712;
    mmr.current.leaderboard_placement = Some(32);
    let r = normalize::rank(&mmr, &TierTable::default());
    assert_eq!(r.rr, 712);
    assert_eq!(r.leaderboard, Some(32));
    mmr.current.tier.id = 0;
    assert_eq!(
        normalize::rank(&mmr, &TierTable::default()).tier_name,
        "Unranked"
    );
    assert_eq!(tiers::accent("b489c4ff"), "#b489c4");
    assert_eq!(tiers::accent("bad"), "#FF4655");
}
#[test]
fn fixtures_and_real_responses_normalize() {
    let mmr = serde_json::from_str::<Envelope<Mmr>>(include_str!("fixtures/mmr.json"))
        .unwrap()
        .data;
    let h = serde_json::from_str::<Envelope<History>>(include_str!("fixtures/history.json"))
        .unwrap()
        .data;
    let mut m = serde_json::from_str::<Envelope<Match>>(include_str!("fixtures/match.json"))
        .unwrap()
        .data;
    let r = normalize::match_entry(&h.history[0], &m, &mmr.account.puuid).unwrap();
    assert_eq!(r.result, Outcome::Win);
    assert_eq!(r.score, "13-9");
    m.teams[0].won = false;
    m.teams[1].won = true;
    assert_eq!(
        normalize::match_entry(&h.history[0], &m, &mmr.account.puuid)
            .unwrap()
            .result,
        Outcome::Loss
    ); // RR sign remains positive
    m.teams[1].won = false;
    assert_eq!(
        normalize::match_entry(&h.history[0], &m, &mmr.account.puuid)
            .unwrap()
            .result,
        Outcome::Draw
    );
    assert!(normalize::match_entry(&h.history[0], &m, "absent").is_err());
    let real_mmr = serde_json::from_str::<Envelope<Mmr>>(include_str!("fixtures/real/mmr.json"))
        .unwrap()
        .data;
    let real_h =
        serde_json::from_str::<Envelope<History>>(include_str!("fixtures/real/history.json"))
            .unwrap()
            .data;
    let real_m = serde_json::from_str::<Envelope<Match>>(include_str!("fixtures/real/match.json"))
        .unwrap()
        .data;
    let real =
        normalize::match_entry(&real_h.history[0], &real_m, &real_mmr.account.puuid).unwrap();
    assert!(!real.agent_id.is_empty());
    assert!(!real.map_id.is_empty());
}
#[test]
fn history_and_events_are_bounded() {
    let matches = (0..70)
        .map(|i| game(&format!("m{i}"), 1600, 0, Outcome::Draw, NOW + i * 1000))
        .collect();
    let s = apply(
        &TrackerState::default(),
        input(rank(18, 1600), matches),
        NOW,
    );
    assert_eq!(s.history.len(), 50);
    assert_eq!(s.events.len(), 20);
    assert_eq!(s.history[0].match_id, "m69");
}

#[test]
fn peak_never_regresses_when_upstream_lags() {
    let prev = baseline();
    let up = apply(&prev, input(rank(20, 1710), vec![]), NOW);
    let mut next = input(rank(19, 1690), vec![]);
    next.peak = Some(PeakView {
        tier_id: 18,
        tier_name: "Diamond 1".into(),
        season: "e9a2".into(),
    });
    let down = apply(&up, next, NOW + 1000);
    assert_eq!(down.peak.unwrap().tier_id, 20);
}

#[test]
fn real_match_performance() {
    let mmr = serde_json::from_str::<Envelope<Mmr>>(include_str!("fixtures/real/mmr.json"))
        .unwrap()
        .data;
    let m = serde_json::from_str::<Envelope<Match>>(include_str!("fixtures/real/match.json"))
        .unwrap()
        .data;
    // Independently computed from the fixture: 24 rounds, 6496 score, 4236 damage, 20/45/3 shots.
    let p = normalize::perf(&m, &mmr.account.puuid).unwrap();
    assert_eq!((p.acs, p.adr, p.hs_pct, p.first_bloods), (271, 177, 29, 1));
    assert_eq!(p.mvp, Some(Mvp::Team)); // top of own team, not of the lobby
    assert!(normalize::perf(&m, "absent").is_none());
}
#[test]
fn perf_mvp_and_empty_matches() {
    let mut m = serde_json::from_str::<Envelope<Match>>(include_str!("fixtures/match.json"))
        .unwrap()
        .data;
    // A lone player is never MVP; zero shots give 0% HS instead of dividing by zero.
    let p = normalize::perf(&m, "mock-ishq").unwrap();
    assert_eq!((p.acs, p.hs_pct, p.mvp), (245, 0, None));
    let mut rival = m.players[0].clone();
    rival.puuid = "rival".into();
    rival.team_id = "Blue".into();
    rival.stats.score = 100;
    m.players.push(rival);
    assert_eq!(
        normalize::perf(&m, "mock-ishq").unwrap().mvp,
        Some(Mvp::Match)
    );
    m.teams[0].rounds = Rounds { won: 0, lost: 0 };
    assert!(normalize::perf(&m, "mock-ishq").is_none());
}
#[test]
fn streak_milestones_fire_once_and_draw_breaks() {
    let mut s = baseline(); // one win
    for i in 1..=4 {
        let at = NOW + i * 1000;
        s = apply(
            &s,
            input(
                rank(18, 1590 + i as i32 * 20),
                vec![game(
                    &format!("w{i}"),
                    1590 + i as i32 * 20,
                    20,
                    Outcome::Win,
                    at,
                )],
            ),
            at,
        );
    }
    assert_eq!(
        s.streak,
        Some(Streak {
            result: Outcome::Win,
            count: 5
        })
    );
    let streaks: Vec<_> = s
        .events
        .iter()
        .filter(|e| e.kind == EventKind::WinStreak)
        .map(|e| e.streak)
        .collect();
    assert_eq!(streaks, vec![Some(3), Some(5)]);
    let again = apply(
        &s,
        input(s.rank.clone(), vec![s.history[0].clone()]),
        NOW + 5000,
    );
    assert_eq!(
        again.events, s.events,
        "replayed games add no streak events"
    );
    let s = apply(
        &s,
        input(
            s.rank.clone(),
            vec![game("d", 1670, 0, Outcome::Draw, NOW + 6000)],
        ),
        NOW + 6000,
    );
    assert_eq!(s.streak, None);
    let mut s = s;
    for i in 0..3 {
        let at = NOW + 7000 + i * 1000;
        s = apply(
            &s,
            input(
                s.rank.clone(),
                vec![game(&format!("l{i}"), 1650, -20, Outcome::Loss, at)],
            ),
            at,
        );
    }
    assert_eq!(s.streak.as_ref().map(|x| x.count), Some(3));
    assert_eq!(
        s.events.last().map(|e| (&e.kind, e.streak)),
        Some((&EventKind::LossStreak, Some(3)))
    );
}
#[test]
fn chat_replies() {
    use tracker_core::text::{reply, KINDS};
    let empty = TrackerState::default();
    assert_eq!(reply(&empty, "lastgame").unwrap(), "No games tracked yet");
    assert_eq!(reply(&empty, "stats").unwrap(), "No match stats yet");
    assert!(reply(&empty, "nope").is_none());
    let mut s = baseline();
    let m = &mut s.history[0];
    (m.map, m.agent, m.score, m.kda) = ("Ascent".into(), "Jett".into(), "13-9".into(), [21, 14, 6]);
    m.perf = Some(Perf {
        acs: 271,
        adr: 177,
        hs_pct: 29,
        first_bloods: 1,
        mvp: Some(Mvp::Team),
    });
    s.rank.tier_name = "Gold 2".into();
    s.rank.rr = 52;
    s.rank.last_change = -20;
    assert_eq!(reply(&s, "rank").unwrap(), "Gold 2 · 52 RR · -20 last game");
    assert_eq!(
        reply(&s, "lastgame").unwrap(),
        "Last game: Win 13-9 on Ascent as Jett · 21/14/6 · 271 ACS · 29% HS · +20 RR · Team MVP"
    );
    assert_eq!(
        reply(&s, "winrate").unwrap(),
        "Last 1: 1W 0L 0D · 100% win rate"
    );
    assert_eq!(
        reply(&s, "stats").unwrap(),
        "Last 1 avg: 271 ACS · 177 ADR · 29% HS · 1.50 K/D"
    );
    assert_eq!(reply(&s, "streak").unwrap(), "No active streak");
    for kind in KINDS {
        let text = reply(&s, kind).unwrap();
        assert!(
            !text.is_empty() && text.len() < 400 && !text.contains('\n'),
            "{kind}"
        );
    }
}
