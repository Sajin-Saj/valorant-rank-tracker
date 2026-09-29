use worker::*;

pub struct Config {
    pub name: String,
    pub tag: String,
    pub region: String,
    pub platform: String,
    pub refresh_secs: u64,
    pub watch_secs: u64,
    pub session_gap_hours: u64,
    pub mock: bool,
    pub key: String,
    pub admin: String,
}
impl Config {
    pub fn from_env(env: &Env) -> Self {
        let value = |name: &str, default: &str| {
            env.secret(name)
                .map(|v| v.to_string())
                .or_else(|_| env.var(name).map(|v| v.to_string()))
                .unwrap_or_else(|_| default.into())
        };
        let number = |name: &str, default: u64| {
            value(name, "")
                .parse::<u64>()
                .ok()
                .filter(|v| *v > 0)
                .unwrap_or(default)
        };
        Self {
            name: value("RIOT_NAME", "ISHQ"),
            tag: value("RIOT_TAG", "tejo2"),
            region: value("REGION", "ap"),
            platform: value("PLATFORM", "pc"),
            refresh_secs: number("REFRESH_SECS", 60).clamp(5, 3600),
            watch_secs: number("WATCH_SECS", 600).clamp(10, 86400),
            session_gap_hours: number("SESSION_GAP_HOURS", 6).min(168),
            mock: value("MOCK_MODE", "false") == "true",
            key: value("HENRIK_API_KEY", ""),
            admin: value("ADMIN_TOKEN", ""),
        }
    }
    pub fn core(&self) -> tracker_core::tracker::Config {
        tracker_core::tracker::Config {
            session_gap_ms: self.session_gap_hours as i64 * 3_600_000,
            watch_ms: self.watch_secs as i64 * 1000,
        }
    }
}
