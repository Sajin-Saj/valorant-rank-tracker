use serde::{de::DeserializeOwned, Serialize};
use worker::{Result, State};

pub async fn get<T: DeserializeOwned>(state: &State, key: &str) -> Result<Option<T>> {
    state.storage().get(key).await
}
pub async fn put<T: Serialize>(state: &State, key: &str, value: &T) -> Result<()> {
    state.storage().put(key, value).await
}

#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Backoff {
    pub until: i64,
    pub step: u32,
    pub status: Option<u16>,
}
impl Backoff {
    pub fn next(&self, now: i64, status: Option<u16>, retry_secs: Option<u64>) -> Self {
        let step = (self.step + 1).min(3);
        let secs = match step {
            1 => 60,
            2 => 120,
            _ => 300,
        }
        .max(retry_secs.unwrap_or(0).min(3600));
        Self {
            until: now + secs as i64 * 1000,
            step,
            status,
        }
    }
}
#[derive(Clone, Default, Serialize, serde::Deserialize)]
pub struct TierCache {
    pub table: tracker_core::tiers::TierTable,
    pub fetched_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backoff_escalates_caps_and_respects_retry_after() {
        let one = Backoff::default().next(1000, Some(429), None);
        assert_eq!(one.until, 61000);
        let two = one.next(61000, Some(503), None);
        assert_eq!(two.until, 181000);
        let three = two.next(181000, Some(500), None);
        assert_eq!(three.until, 481000);
        let capped = three.next(481000, None, None);
        assert_eq!(capped.step, 3);
        assert_eq!(capped.until, 781000);
        assert_eq!(
            Backoff::default().next(0, Some(429), Some(180)).until,
            180000
        );
    }
}
