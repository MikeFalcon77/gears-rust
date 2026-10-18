//! Explicit demo `AuthN` composition. JWT-shaped credentials never fall back to static auth.
pub mod config;
mod service;
use async_trait::async_trait;
use authn_resolver_sdk::{AuthNResolverPluginClient, AuthNResolverPluginSpecV1};
use std::sync::Arc;
use toolkit::client_hub::ClientScope;
use toolkit::{Gear, GearCtx};
use toolkit_gts::PluginV1;
use types_registry_sdk::{RegisterResult, TypesRegistryClient};

#[toolkit::gear(name = "demo-authn-plugin", deps = [types_registry, firebase_authn_plugin, static_authn_plugin])]
#[derive(Default)]
pub struct DemoAuthnPlugin;

#[async_trait]
impl Gear for DemoAuthnPlugin {
    async fn init(&self, ctx: &GearCtx) -> anyhow::Result<()> {
        let config: config::DemoAuthnPluginConfig = ctx.config_or_default()?;
        let firebase_id =
            AuthNResolverPluginSpecV1::gts_make_instance_id("cf.builtin.firebase_authn.plugin.v1");
        let technical_id = AuthNResolverPluginSpecV1::gts_make_instance_id(
            "cf.builtin.static_authn_resolver.plugin.v1",
        );
        let firebase = ctx
            .client_hub()
            .get_scoped::<dyn AuthNResolverPluginClient>(&ClientScope::gts_id(&firebase_id))?;
        let technical = ctx
            .client_hub()
            .get_scoped::<dyn AuthNResolverPluginClient>(&ClientScope::gts_id(&technical_id))?;
        let (id, registration) = PluginV1::<AuthNResolverPluginSpecV1>::build_registration(
            "cf.demo.authn_adapter.plugin.v1",
            config.vendor,
            config.priority,
        )?;
        let registry = ctx.client_hub().get::<dyn TypesRegistryClient>()?;
        RegisterResult::ensure_all_ok(&registry.register(vec![registration]).await?)?;
        ctx.client_hub()
            .register_scoped::<dyn AuthNResolverPluginClient>(
                ClientScope::gts_id(&id),
                Arc::new(service::Service::new(firebase, technical)),
            );
        Ok(())
    }
}
