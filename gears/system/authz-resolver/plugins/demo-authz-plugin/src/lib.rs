//! Explicit demo action policy, delegating tenant hierarchy decisions to tr-authz.
pub mod config;
mod service;
use async_trait::async_trait;
use authz_resolver_sdk::{AuthZResolverPluginClient, AuthZResolverPluginSpecV1};
use std::sync::Arc;
use toolkit::client_hub::ClientScope;
use toolkit::{Gear, GearCtx};
use toolkit_gts::PluginV1;
use types_registry_sdk::{RegisterResult, TypesRegistryClient};

#[toolkit::gear(name = "demo-authz-plugin", deps = [types_registry, tr_authz_plugin])]
#[derive(Default)]
pub struct DemoAuthzPlugin;
#[async_trait]
impl Gear for DemoAuthzPlugin {
    async fn init(&self, ctx: &GearCtx) -> anyhow::Result<()> {
        let config: config::DemoAuthzPluginConfig = ctx.config_or_default()?;
        config.validate()?;
        let tr_id = AuthZResolverPluginSpecV1::gts_make_instance_id(
            "cf.builtin.tr_authz_resolver.plugin.v1",
        );
        let hierarchy = ctx
            .client_hub()
            .get_scoped::<dyn AuthZResolverPluginClient>(&ClientScope::gts_id(&tr_id))?;
        let (id, registration) = PluginV1::<AuthZResolverPluginSpecV1>::build_registration(
            "cf.demo.action_policy.plugin.v1",
            config.vendor.clone(),
            config.priority,
        )?;
        let registry = ctx.client_hub().get::<dyn TypesRegistryClient>()?;
        RegisterResult::ensure_all_ok(&registry.register(vec![registration]).await?)?;
        ctx.client_hub()
            .register_scoped::<dyn AuthZResolverPluginClient>(
                ClientScope::gts_id(&id),
                Arc::new(service::Service::new(config, hierarchy)),
            );
        Ok(())
    }
}
