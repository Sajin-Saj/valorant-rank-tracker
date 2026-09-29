use crate::config::Config;
use serde::de::DeserializeOwned;
use tracker_core::{
    henrik::{Envelope, History, Match, Mmr},
    tiers::TierTable,
};
use worker::*;

pub struct Failure {
    pub status: Option<u16>,
    pub retry_secs: Option<u64>,
}
impl From<worker::Error> for Failure {
    fn from(_: worker::Error) -> Self {
        Self {
            status: None,
            retry_secs: None,
        }
    }
}
pub fn segment(s: &str) -> String {
    worker::Url::parse("https://example.com")
        .map(|mut u| {
            u.path_segments_mut().unwrap().push(s);
            u.path().trim_start_matches('/').to_owned()
        })
        .unwrap_or_default()
}
pub async fn json<T: DeserializeOwned>(
    url: &str,
    key: Option<&str>,
) -> std::result::Result<T, Failure> {
    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    if let Some(k) = key {
        headers.set("Authorization", k)?;
    }
    let mut init = RequestInit::new();
    init.with_headers(headers).with_method(Method::Get);
    let req = Request::new_with_init(url, &init)?;
    let mut response = Fetch::Request(req).send().await?;
    if response.status_code() != 200 {
        return Err(Failure {
            status: Some(response.status_code()),
            retry_secs: response
                .headers()
                .get("Retry-After")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok()),
        });
    }
    response.json().await.map_err(Failure::from)
}
pub async fn mmr(cfg: &Config, puuid: Option<&str>) -> std::result::Result<Mmr, Failure> {
    let path = match puuid {
        Some(id) => format!(
            "v3/by-puuid/mmr/{}/{}/{}",
            cfg.region,
            cfg.platform,
            segment(id)
        ),
        None => format!(
            "v3/mmr/{}/{}/{}/{}",
            cfg.region,
            cfg.platform,
            segment(&cfg.name),
            segment(&cfg.tag)
        ),
    };
    let response: Envelope<Mmr> = json(
        &format!("https://api.henrikdev.xyz/valorant/{path}"),
        Some(&cfg.key),
    )
    .await?;
    if response.data.account.puuid.is_empty() || response.data.current.tier.name.is_empty() {
        return Err(Failure {
            status: Some(502),
            retry_secs: None,
        });
    }
    Ok(response.data)
}
pub async fn history(cfg: &Config, puuid: &str) -> std::result::Result<History, Failure> {
    let response: Envelope<History> = json(
        &format!(
            "https://api.henrikdev.xyz/valorant/v2/by-puuid/mmr-history/{}/{}/{}",
            cfg.region,
            cfg.platform,
            segment(puuid)
        ),
        Some(&cfg.key),
    )
    .await?;
    Ok(response.data)
}
pub async fn game(cfg: &Config, id: &str) -> std::result::Result<Match, Failure> {
    let response: Envelope<Match> = json(
        &format!(
            "https://api.henrikdev.xyz/valorant/v4/match/{}/{}",
            cfg.region,
            segment(id)
        ),
        Some(&cfg.key),
    )
    .await?;
    Ok(response.data)
}
pub async fn tiers() -> std::result::Result<TierTable, Failure> {
    let response: Envelope<Vec<TierTable>> =
        json("https://valorant-api.com/v1/competitivetiers", None).await?;
    response.data.into_iter().last().ok_or(Failure {
        status: Some(502),
        retry_secs: None,
    })
}
