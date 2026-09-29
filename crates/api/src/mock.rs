use tracker_core::{
    henrik::{Envelope, History, Match, Mmr},
    model::*,
    normalize,
    tiers::TierTable,
    tracker::{self, RefreshInput},
};

pub fn refresh(
    prev: &TrackerState,
    step: u64,
    now: i64,
    cfg: tracker::Config,
    tiers: &TierTable,
) -> TrackerState {
    let mut mmr: Mmr =
        serde_json::from_str::<Envelope<Mmr>>(include_str!("../../core/tests/fixtures/mmr.json"))
            .unwrap()
            .data;
    let mut h = serde_json::from_str::<Envelope<History>>(include_str!(
        "../../core/tests/fixtures/history.json"
    ))
    .unwrap()
    .data
    .history
    .remove(0);
    let mut game = serde_json::from_str::<Envelope<Match>>(include_str!(
        "../../core/tests/fixtures/match.json"
    ))
    .unwrap()
    .data;
    let cycle = step % 5;
    let (tier, name, rr, change, win, loss) = match cycle {
        0 => (19, "Diamond 2", 67, 21, true, false),
        1 => (19, "Diamond 2", 51, -16, false, true),
        2 => (20, "Diamond 3", 10, 59, true, false),
        3 => (20, "Diamond 3", 10, 0, false, false),
        _ => (19, "Diamond 2", 90, -20, false, true),
    };
    mmr.current.tier.id = tier;
    mmr.current.tier.name = name.into();
    mmr.current.rr = rr;
    mmr.current.last_change = change;
    mmr.current.elo = (tier as i32 - 3) * 100 + rr;
    h.match_id = format!("mock-{}-{}", now, step);
    h.date = tracker::timestamp(now);
    h.rr = rr;
    h.elo = mmr.current.elo;
    h.last_change = change;
    h.tier = mmr.current.tier.clone();
    game.metadata.match_id = h.match_id.clone();
    game.metadata.started_at = h.date.clone();
    game.teams[0].won = win;
    game.teams[1].won = loss;
    game.teams[0].rounds.won = if loss {
        9
    } else if win {
        13
    } else {
        14
    };
    game.teams[1].rounds.won = if win {
        9
    } else if loss {
        13
    } else {
        14
    };
    let entry = normalize::match_entry(&h, &game, "mock-ishq").unwrap();
    tracker::apply_refresh(
        prev,
        RefreshInput {
            rank: normalize::rank(&mmr, tiers),
            peak: normalize::peak(&mmr),
            acts: normalize::acts(&mmr, tiers),
            matches: vec![entry],
            source: "mock".into(),
        },
        now,
        cfg,
    )
}
