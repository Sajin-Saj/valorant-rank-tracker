use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TierAsset {
    pub tier: u32,
    pub tier_name: String,
    pub large_icon: Option<String>,
    pub small_icon: Option<String>,
    pub color: String,
    pub background_color: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TierTable {
    pub tiers: Vec<TierAsset>,
}

pub fn accent(color: &str) -> String {
    let hex = color.trim_start_matches('#');
    if (hex.len() == 6 || hex.len() == 8) && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("#{}", &hex[..6])
    } else {
        "#FF4655".into()
    }
}
impl TierTable {
    pub fn asset(&self, id: u32) -> Option<&TierAsset> {
        self.tiers.iter().find(|t| t.tier == id)
    }
    pub fn icon(&self, id: u32) -> String {
        self.asset(id)
            .and_then(|t| t.large_icon.clone().or(t.small_icon.clone()))
            .unwrap_or_default()
    }
    pub fn color(&self, id: u32) -> String {
        self.asset(id)
            .map(|t| accent(&t.color))
            .unwrap_or_else(|| "#FF4655".into())
    }
}
