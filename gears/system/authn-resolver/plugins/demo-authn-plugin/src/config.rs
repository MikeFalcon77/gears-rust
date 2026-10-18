//! This adapter is selected once by `AuthN` resolver; delegates are explicit SDK scopes.
use serde::Deserialize;
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DemoAuthnPluginConfig {
    pub vendor: String,
    pub priority: i16,
}
impl Default for DemoAuthnPluginConfig {
    fn default() -> Self {
        Self {
            vendor: "constructorfabric".to_owned(),
            priority: 0,
        }
    }
}
