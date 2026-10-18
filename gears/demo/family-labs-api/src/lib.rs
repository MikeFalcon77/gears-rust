//! Gateway adapter consuming the authorized Settings contract over REST.
use async_trait::async_trait;
use axum::{Extension, Json, Router};
use settings_service_sdk::{
    SettingsReadApi,
    models::{EffectiveValueResponse, GetEffectiveRequest},
    remote::{BulkEffectiveRequest, RemoteBulkOutcome},
};
use std::sync::Arc;
use toolkit::api::{OpenApiRegistry, canonical_prelude::*, operation_builder::OperationBuilder};
use toolkit::client_hub::ClientHub;
use toolkit::{Gear, GearCtx, RestApiCapability};
use toolkit_security::SecurityContext;

#[toolkit::gear(name = "family-labs-api", deps = [oagw, types_registry], capabilities = [rest])]
#[toolkit::consumes(contract = settings_service_sdk::SettingsReadApi, from = "settings-service")]
#[derive(Default)]
pub struct FamilyLabsApi;
#[async_trait]
impl Gear for FamilyLabsApi {
    async fn init(&self, ctx: &GearCtx) -> anyhow::Result<()> {
        let types = ctx
            .client_hub()
            .get::<dyn types_registry_sdk::TypesRegistryClient>()?;
        types_registry_sdk::RegisterResult::ensure_all_ok(
            &types.register(echo_declarations()).await?,
        )?;
        Ok(())
    }
}
async fn effective(
    Extension(ctx): Extension<SecurityContext>,
    Extension(hub): Extension<Arc<ClientHub>>,
    Json(req): Json<GetEffectiveRequest>,
) -> ApiResult<Json<EffectiveValueResponse>> {
    let client = hub.get::<dyn SettingsReadApi>().map_err(|_| {
        toolkit_canonical_errors::CanonicalError::service_unavailable()
            .with_detail("Settings contract is unavailable")
            .create()
    })?;
    Ok(Json(client.get_effective(ctx, req).await?))
}
async fn bulk(
    Extension(ctx): Extension<SecurityContext>,
    Extension(hub): Extension<Arc<ClientHub>>,
    Json(req): Json<BulkEffectiveRequest>,
) -> ApiResult<Json<Vec<RemoteBulkOutcome>>> {
    let client = hub.get::<dyn SettingsReadApi>().map_err(|_| {
        toolkit_canonical_errors::CanonicalError::service_unavailable()
            .with_detail("Settings contract is unavailable")
            .create()
    })?;
    Ok(Json(client.get_effective_bulk(ctx, req).await?))
}
impl RestApiCapability for FamilyLabsApi {
    fn register_rest(
        &self,
        ctx: &GearCtx,
        router: Router,
        openapi: &dyn OpenApiRegistry,
    ) -> anyhow::Result<Router> {
        let router = OperationBuilder::post("/family-labs/v1/effective:read")
            .operation_id("family_labs.effective")
            .summary("Read effective public settings through the worker contract")
            .tag("Family Labs")
            .authenticated()
            .no_license_required()
            .exposed()
            .json_request::<GetEffectiveRequest>(openapi, "Setting key and target scope")
            .handler(effective)
            .json_response_with_schema::<EffectiveValueResponse>(
                openapi,
                StatusCode::OK,
                "Effective value",
            )
            .error_400(openapi)
            .error_401(openapi)
            .error_403(openapi)
            .error_404(openapi)
            .error_503(openapi)
            .register(router, openapi);
        Ok(OperationBuilder::post("/family-labs/v1/effective:readBulk")
            .operation_id("family_labs.effective_bulk")
            .summary("Read a bounded set of public settings through the worker contract")
            .tag("Family Labs")
            .authenticated()
            .no_license_required()
            .exposed()
            .json_request::<BulkEffectiveRequest>(openapi, "Keys and target scope")
            .handler(bulk)
            .json_array_response_with_schema::<RemoteBulkOutcome>(
                openapi,
                StatusCode::OK,
                "Per-key outcomes",
            )
            .error_400(openapi)
            .error_401(openapi)
            .error_403(openapi)
            .error_503(openapi)
            .register(router, openapi)
            .layer(Extension(ctx.client_hub())))
    }
}

// OAGW materializes these SDK-owned declarations in post_init, after every gear
// finished init. Per-lab upstreams preserve concrete owner-tenant authorization.
fn echo_declarations() -> Vec<serde_json::Value> {
    let mut entities = Vec::new();
    for (tenant, upstream, route) in [
        (
            "d0000000-0000-4000-8000-000000000002",
            "d0000000-0000-4000-8000-000000000101",
            "d0000000-0000-4000-8000-000000000102",
        ),
        (
            "a0000000-0000-4000-8000-000000000003",
            "a0000000-0000-4000-8000-000000000101",
            "a0000000-0000-4000-8000-000000000102",
        ),
    ] {
        entities.push(serde_json::json!({
            "id": format!("gts.cf.core.oagw.upstream.v1~{upstream}"), "tenant_id": tenant,
            "server": {"endpoints": [{"scheme": "http", "host": "demo-echo.gearbox-demo.svc.cluster.local", "port": 8080}]},
            "protocol": "gts.cf.core.oagw.protocol.v1~cf.core.oagw.http.v1",
            "auth": {"type": "gts.cf.core.oagw.auth_plugin.v1~cf.core.oagw.noop.v1"},
            "headers": {"request": {"passthrough": "allowlist", "passthrough_allowlist": ["x-request-id"], "remove": ["authorization", "cookie"]}},
            "tags": ["family-labs"], "enabled": true
        }));
        entities.push(serde_json::json!({
            "id": format!("gts.cf.core.oagw.route.v1~{route}"), "tenant_id": tenant,
            "upstream_id": format!("gts.cf.core.oagw.upstream.v1~{upstream}"),
            "match": {"http": {"methods": ["GET"], "path": "/echo", "path_suffix_mode": "disabled"}},
            "enabled": true, "tags": ["family-labs"], "priority": 0
        }));
    }
    entities
}
