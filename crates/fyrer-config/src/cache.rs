use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CacheConfig {
    #[serde(default)]
    pub provider: CacheProviderKind,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CacheProviderKind {
    #[default]
    Local,
}
