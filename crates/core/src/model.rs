use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RankView {
    pub tier_id: u32,
    pub tier_name: String,
    pub rr: i32,
    pub elo: i32,
    pub last_change: i32,
    pub icon: String,
    pub accent: String,
    pub leaderboard: Option<u32>,
}
impl Default for RankView {
    fn default() -> Self {
        Self {
            tier_id: 0,
            tier_name: "Unranked".into(),
            rr: 0,
            elo: 0,
            last_change: 0,
            icon: String::new(),
            accent: "#FF4655".into(),
            leaderboard: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PeakView {
    pub tier_id: u32,
    pub tier_name: String,
    pub season: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActView {
    pub season: String,
    pub end_tier: String,
    pub tier_id: u32,
    pub icon: String,
    pub wins: u32,
    pub games: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Win,
    Loss,
    #[default]
    Draw,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MatchEntry {
    pub match_id: String,
    pub map: String,
    pub map_id: String,
    pub agent: String,
    pub agent_id: String,
    pub result: Outcome,
    pub score: String,
    pub kda: [u32; 3],
    pub rr_change: i32,
    pub rr_after: i32,
    pub elo_after: i32,
    pub tier_id: u32,
    pub derank_protected: bool,
    pub refunded_rr: i32,
    pub at: String,
    pub perf: Option<Perf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mvp {
    Match,
    Team,
}

/// Per-game performance derived from the v4 match; `None` for games stored before stats existed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Perf {
    pub acs: u32,
    pub adr: u32,
    pub hs_pct: u32,
    pub first_bloods: u32,
    pub mvp: Option<Mvp>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Streak {
    pub result: Outcome,
    pub count: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub started_at: String,
    pub start_elo: i32,
    pub net_rr: i32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    MatchResult,
    RankUp,
    Derank,
    NewPeak,
    SessionStart,
    WinStreak,
    LossStreak,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: u64,
    pub kind: EventKind,
    pub at: String,
    pub quiet: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Outcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rr_change: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streak: Option<u32>,
    pub icon: String,
    pub accent: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub checked_at: String,
    pub changed_at: String,
    pub stale: bool,
    pub source: String,
    pub catching_up: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackerState {
    pub rank: RankView,
    pub peak: Option<PeakView>,
    pub acts: Vec<ActView>,
    pub history: Vec<MatchEntry>,
    pub session: Option<Session>,
    pub streak: Option<Streak>,
    pub events: Vec<Event>,
    pub next_event_id: u64,
    pub meta: Meta,
}
impl Default for TrackerState {
    fn default() -> Self {
        Self {
            rank: RankView::default(),
            peak: None,
            acts: vec![],
            history: vec![],
            session: None,
            streak: None,
            events: vec![],
            next_event_id: 1,
            meta: Meta::default(),
        }
    }
}
