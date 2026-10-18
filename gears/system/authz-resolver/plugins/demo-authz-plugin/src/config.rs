//! Explicit demo grants. Unknown actions/resources and unknown tenants fail closed.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DemoAuthzPluginConfig {
    pub vendor: String,
    pub priority: i16,
    pub allowed_tenants: BTreeSet<Uuid>,
    pub grants: Vec<Grant>,
}
impl Default for DemoAuthzPluginConfig {
    fn default() -> Self {
        Self {
            vendor: "constructorfabric".to_owned(),
            priority: 0,
            allowed_tenants: BTreeSet::new(),
            grants: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub scope: String,
    pub resource_type: String,
    pub actions: BTreeSet<String>,
    #[serde(default)]
    pub subject_ids: BTreeSet<Uuid>,
    #[serde(default)]
    pub subject_tenants: BTreeSet<Uuid>,
    /// Every configured property must be present and match one exact allowed value.
    #[serde(default)]
    pub properties: BTreeMap<String, BTreeSet<String>>,
}
impl DemoAuthzPluginConfig {
    /// Validate the bounded demo policy before it is registered.
    /// # Errors
    /// Refuses an empty tenant/grant set or an unbounded wildcard grant.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.allowed_tenants.is_empty() && self.allowed_tenants.iter().all(|id| !id.is_nil()),
            "allowed demo tenants are required"
        );
        anyhow::ensure!(!self.grants.is_empty(), "demo grants are required");
        for grant in &self.grants {
            anyhow::ensure!(
                !grant.scope.is_empty()
                    && !grant.actions.is_empty()
                    && !grant.resource_type.is_empty(),
                "grant scope, resource and actions are required"
            );
            if grant.resource_type == "*" || grant.actions.contains("*") {
                anyhow::ensure!(
                    !grant.subject_ids.is_empty() && !grant.subject_tenants.is_empty(),
                    "wildcard grants require explicit subject and tenant identities"
                );
            }
            anyhow::ensure!(
                grant.properties.values().all(|values| !values.is_empty()),
                "property allowlists must not be empty"
            );
        }
        Ok(())
    }
}
