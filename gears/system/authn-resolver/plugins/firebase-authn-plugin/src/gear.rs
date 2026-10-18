//! Firebase `AuthN` lifecycle and its scoped SDK implementation.
use crate::config::FirebaseAuthnPluginConfig;
use crate::firebase::{FirebaseVerifier, Refusal};
use async_trait::async_trait;
use authn_resolver_sdk::{
    AuthNResolverError, AuthNResolverPluginClient, AuthNResolverPluginSpecV1, AuthenticationResult,
    ClientCredentialsRequest,
};
use secrecy::SecretString;
use std::sync::{Arc, OnceLock};
use toolkit::client_hub::ClientScope;
use toolkit::{Gear, GearCtx};
use toolkit_gts::PluginV1;
use toolkit_security::SecurityContext;
use types_registry_sdk::{RegisterResult, TypesRegistryClient};
use uuid::Uuid;

pub const PLUGIN_SUFFIX: &str = "cf.builtin.firebase_authn.plugin.v1";

#[toolkit::gear(name = "firebase-authn-plugin", deps = [types_registry])]
#[derive(Default)]
pub struct FirebaseAuthnPlugin {
    service: OnceLock<Arc<FirebaseService>>,
}

#[async_trait]
impl Gear for FirebaseAuthnPlugin {
    async fn init(&self, ctx: &GearCtx) -> anyhow::Result<()> {
        let config: FirebaseAuthnPluginConfig = ctx.config_expanded_or_default()?;
        config.validate()?;
        let verifier = FirebaseVerifier::new(
            config.project_id.clone(),
            config
                .identities
                .iter()
                .map(|identity| (identity.uid.clone(), identity.email.clone()))
                .collect(),
        )?;
        let (id, registration) = PluginV1::<AuthNResolverPluginSpecV1>::build_registration(
            PLUGIN_SUFFIX,
            config.vendor.clone(),
            config.priority,
        )?;
        let registry = ctx.client_hub().get::<dyn TypesRegistryClient>()?;
        RegisterResult::ensure_all_ok(&registry.register(vec![registration]).await?)?;
        let service = Arc::new(FirebaseService { config, verifier });
        self.service
            .set(service.clone())
            .map_err(|_| anyhow::anyhow!("Firebase plugin already initialized"))?;
        ctx.client_hub()
            .register_scoped::<dyn AuthNResolverPluginClient>(ClientScope::gts_id(&id), service);
        Ok(())
    }
}

struct FirebaseService {
    config: FirebaseAuthnPluginConfig,
    verifier: FirebaseVerifier,
}

#[async_trait]
impl AuthNResolverPluginClient for FirebaseService {
    async fn authenticate(&self, token: &str) -> Result<AuthenticationResult, AuthNResolverError> {
        let uid = self
            .verifier
            .verify_identity(token)
            .await
            .map_err(|refusal| match refusal {
                Refusal::KeysUnavailable(_) => AuthNResolverError::ServiceUnavailable(
                    "Firebase signing keys unavailable".to_owned(),
                ),
                _ => AuthNResolverError::Unauthorized("Firebase identity not permitted".to_owned()),
            })?;
        let identity = self
            .config
            .identities
            .iter()
            .find(|identity| identity.uid == uid)
            .ok_or_else(|| {
                AuthNResolverError::Unauthorized("Firebase identity not permitted".to_owned())
            })?;
        let subject = Uuid::new_v5(
            &self.config.subject_namespace,
            format!("firebase://{}/{}", self.config.project_id, uid).as_bytes(),
        );
        let security_context = SecurityContext::builder()
            .subject_id(subject)
            .subject_tenant_id(identity.tenant_id)
            .subject_type("user")
            .token_scopes(identity.token_scopes.clone())
            .bearer_token(SecretString::from(token.to_owned()))
            .build()
            .map_err(|error| AuthNResolverError::Internal(error.to_string()))?;
        Ok(AuthenticationResult { security_context })
    }
    async fn exchange_client_credentials(
        &self,
        _: &ClientCredentialsRequest,
    ) -> Result<AuthenticationResult, AuthNResolverError> {
        Err(AuthNResolverError::TokenAcquisitionFailed(
            "Firebase user sign-in required".to_owned(),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "signed token integration fixtures")]
mod tests {
    use super::*;
    use crate::config::IdentityMapping;
    #[tokio::test]
    async fn signed_identity_gets_only_its_configured_tenant_and_scopes() {
        let config = FirebaseAuthnPluginConfig {
            project_id: "firebase-authn-test".to_owned(),
            identities: (1..=3)
                .map(|id| IdentityMapping {
                    uid: format!("account-{id}"),
                    email: format!("account-{id}@example.test"),
                    tenant_id: Uuid::from_u128(id),
                    token_scopes: vec![format!("scope:{id}")],
                })
                .collect(),
            ..Default::default()
        };
        config.validate().unwrap();
        let verifier = crate::firebase::with_test_keys(
            config.project_id.clone(),
            config
                .identities
                .iter()
                .map(|i| (i.uid.clone(), i.email.clone()))
                .collect(),
            crate::firebase::tests::signing_keys(),
        );
        let service = FirebaseService { config, verifier };
        for id in 1..=3 {
            let token = crate::firebase::tests::signed_claims(
                serde_json::json!({"sub": format!("account-{id}"), "email": format!("account-{id}@example.test"), "tenant_id": Uuid::nil(), "scopes": ["*"]}),
            );
            let result = service.authenticate(&token).await.unwrap();
            assert_eq!(
                result.security_context.subject_tenant_id(),
                Uuid::from_u128(id)
            );
            assert_eq!(
                result.security_context.token_scopes(),
                &[format!("scope:{id}")]
            );
            assert!(!result.security_context.subject_id().is_nil());
        }
    }
}
