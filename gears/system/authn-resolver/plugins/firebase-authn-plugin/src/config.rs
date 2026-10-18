//! Deployment-controlled identity mappings. Browser claims never choose a tenant.
use serde::Deserialize;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, toolkit_macros::ExpandVars)]
#[serde(default, deny_unknown_fields)]
pub struct FirebaseAuthnPluginConfig {
    pub vendor: String,
    pub priority: i16,
    #[expand_vars]
    pub project_id: String,
    pub subject_namespace: Uuid,
    #[expand_vars]
    pub identities: Vec<IdentityMapping>,
}

impl Default for FirebaseAuthnPluginConfig {
    fn default() -> Self {
        Self {
            vendor: "constructorfabric".to_owned(),
            priority: 25,
            project_id: String::new(),
            subject_namespace: Uuid::NAMESPACE_URL,
            identities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, toolkit_macros::ExpandVars)]
#[serde(deny_unknown_fields)]
pub struct IdentityMapping {
    #[expand_vars]
    pub uid: String,
    #[expand_vars]
    pub email: String,
    pub tenant_id: Uuid,
    pub token_scopes: Vec<String>,
}

impl FirebaseAuthnPluginConfig {
    /// Validate the deployment allowlist before registering any plugin instance.
    /// # Errors
    /// Refuses missing project/mappings, duplicate UIDs and nil tenant identities.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.project_id.is_empty()
                && self.project_id.len() <= 63
                && self
                    .project_id
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
            "invalid Firebase project ID"
        );
        anyhow::ensure!(
            !self.subject_namespace.is_nil(),
            "subject namespace must not be nil"
        );
        anyhow::ensure!(
            !self.identities.is_empty(),
            "Firebase identity mappings are required"
        );
        let mut uids = BTreeSet::new();
        for identity in &self.identities {
            anyhow::ensure!(
                !identity.uid.is_empty() && identity.uid.len() <= 128 && uids.insert(&identity.uid),
                "invalid or duplicate Firebase UID mapping"
            );
            anyhow::ensure!(
                identity.email.contains('@') && !identity.email.trim().is_empty(),
                "verified email mapping is required"
            );
            anyhow::ensure!(
                !identity.tenant_id.is_nil(),
                "mapped tenant must not be nil"
            );
            anyhow::ensure!(
                !identity.token_scopes.is_empty()
                    && identity
                        .token_scopes
                        .iter()
                        .all(|scope| !scope.trim().is_empty()),
                "identity scopes are required"
            );
        }
        Ok(())
    }
}
