use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Envelope<T> {
    pub status: Option<u16>,
    pub data: T,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Named {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Tier {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Season {
    pub id: String,
    pub short: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Account {
    pub puuid: String,
    pub name: String,
    pub tag: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Current {
    pub tier: Tier,
    pub rr: i32,
    pub last_change: i32,
    pub elo: i32,
    pub leaderboard_placement: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Peak {
    pub tier: Tier,
    pub season: Season,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Seasonal {
    pub season: Season,
    pub end_tier: Tier,
    pub wins: u32,
    pub games: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Mmr {
    pub account: Account,
    pub current: Current,
    pub peak: Option<Peak>,
    pub seasonal: Vec<Seasonal>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct History {
    pub history: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct HistoryEntry {
    pub match_id: String,
    pub tier: Tier,
    pub map: Named,
    pub season: Season,
    pub rr: i32,
    pub last_change: i32,
    pub elo: i32,
    pub refunded_rr: i32,
    pub was_derank_protected: bool,
    pub date: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Match {
    pub metadata: Metadata,
    pub players: Vec<Player>,
    pub teams: Vec<Team>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Metadata {
    pub match_id: String,
    pub map: Named,
    pub started_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Player {
    pub puuid: String,
    pub team_id: String,
    pub agent: Named,
    pub stats: Stats,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Stats {
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub score: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Team {
    pub team_id: String,
    pub won: bool,
    pub rounds: Rounds,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Rounds {
    pub won: u32,
    pub lost: u32,
}
