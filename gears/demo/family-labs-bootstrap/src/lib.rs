//! Trusted startup contribution through the Settings SDK, with no public endpoint.
use async_trait::async_trait;
use authn_resolver_sdk::AuthNResolverClient;
use serde::Deserialize;
use serde_json::json;
use settings_service_sdk::{
    SettingKey,
    api::SettingsContributionClient,
    catalogue::{BOOL_FLAG, STRING},
    models::{ContributedDeclaration, ScopeClass},
};
use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};
use tokio_util::sync::CancellationToken;
use toolkit::client_hub::ClientHub;
use toolkit::lifecycle::ReadySignal;
use toolkit::{Gear, GearCtx};

#[derive(Deserialize, toolkit::ExpandVars)]
#[serde(deny_unknown_fields)]
struct Config {
    #[expand_vars]
    bootstrap_token: String,
}

/// Names, types and defaults are owned by this gear. Tenant values are not seeds.
fn declarations() -> anyhow::Result<Vec<ContributedDeclaration>> {
    let major = NonZeroU32::new(1).ok_or_else(|| anyhow::anyhow!("invalid major"))?;
    [
        (
            "ice_cream_approved",
            BOOL_FLAG,
            json!(false),
            "Approve the ice cream deployment.",
        ),
        (
            "lab_message",
            STRING,
            json!("Ready for experiments."),
            "A friendly message for this laboratory.",
        ),
    ]
    .into_iter()
    .map(|(name, value_type, default, description)| {
        let key = SettingKey::contributed("cf", "family_labs", "lab", name, major)?;
        let mut declaration =
            ContributedDeclaration::new(key, value_type, default, ScopeClass::Local);
        declaration.description = Some(description.to_owned());
        declaration.requires_step_up = Some(false);
        declaration.anonymous_exposable = Some(false);
        Ok(declaration)
    })
    .collect()
}

#[toolkit::gear(name = "family-labs-bootstrap", deps = [settings_service, authn_resolver, demo_catalogue], capabilities = [stateful], lifecycle(entry = "serve", stop_timeout = "10s", await_ready))]
#[derive(Default)]
pub struct FamilyLabsBootstrap {
    state: OnceLock<(Config, Arc<ClientHub>)>,
}

#[async_trait]
impl Gear for FamilyLabsBootstrap {
    async fn init(&self, ctx: &GearCtx) -> anyhow::Result<()> {
        // DB migrations and all plugin initialization must finish before writes.
        let config: Config = ctx.config_expanded()?;
        self.state
            .set((config, ctx.client_hub()))
            .map_err(|_| anyhow::anyhow!("bootstrap already initialized"))?;
        Ok(())
    }
}
impl FamilyLabsBootstrap {
    async fn serve(
        self: Arc<Self>,
        cancel: CancellationToken,
        ready: ReadySignal,
    ) -> anyhow::Result<()> {
        let (config, hub) = self
            .state
            .get()
            .ok_or_else(|| anyhow::anyhow!("bootstrap not initialized"))?;
        let authn = hub.get::<dyn AuthNResolverClient>()?;
        let identity = authn.authenticate(&config.bootstrap_token).await?;
        anyhow::ensure!(
            identity
                .security_context
                .token_scopes()
                .contains(&"demo:bootstrap".to_owned()),
            "bootstrap scope required"
        );
        let settings = hub.get::<dyn SettingsContributionClient>()?;
        let result = settings
            .register_declarations(
                &identity.security_context,
                "family-labs".to_owned(),
                declarations()?,
            )
            .await?;
        anyhow::ensure!(
            result.errors.is_empty(),
            "Family Labs declarations refused: {:?}",
            result.errors
        );
        ready.notify();
        cancel.cancelled().await;
        Ok(())
    }
}
