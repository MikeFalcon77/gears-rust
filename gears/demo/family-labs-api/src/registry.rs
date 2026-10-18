//! Fixed-target, read-only V1 diagnostics. Caller credentials are forwarded only
//! to the two product-owned endpoints; browser input cannot choose a URL.
use axum::{Extension, Json, Router, extract::Path, http::HeaderMap};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use toolkit::api::{OpenApiRegistry, canonical_prelude::*, operation_builder::OperationBuilder};
use toolkit::client_hub::ClientHub;
use toolkit_http::{HttpClient, RedirectConfig, TransportSecurity};
use toolkit_security::SecurityContext;
use types_registry_sdk::{InstanceQuery, TypeSchemaQuery, TypesRegistryClient};

#[toolkit_canonical_errors::resource_error("gts.cf.core.types_registry.entity.v1~")]
struct RegistryError;

pub struct Reader {
    client: HttpClient,
}
impl Reader {
    pub fn new() -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            client: HttpClient::builder()
                .transport(TransportSecurity::AllowInsecureHttp)
                .redirect(RedirectConfig::disabled())
                .retry(None)
                .timeout(Duration::from_secs(8))
                .max_body_size(4 * 1024 * 1024)
                .build()?,
        }))
    }
}
#[derive(Debug, Clone)]
#[toolkit_macros::api_dto(response)]
struct RegistryResponse {
    entities: Vec<Value>,
    count: usize,
}
fn target(source: &str) -> Option<&'static str> {
    match source {
        "settings" => Some("http://demo-settings-worker:8090/types-registry/v1/entities"),
        _ => None,
    }
}
fn visible(entity: &Value, tenant: &str) -> bool {
    entity["is_schema"] == true || entity["content"]["tenant_id"].as_str() == Some(tenant)
}
async fn read(
    Extension(ctx): Extension<SecurityContext>,
    Extension(reader): Extension<Arc<Reader>>,
    Extension(hub): Extension<Arc<ClientHub>>,
    Path(source): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<RegistryResponse>> {
    use toolkit_canonical_errors::CanonicalError;
    if source == "gateway" {
        let registry = hub.get::<dyn TypesRegistryClient>().map_err(|_| {
            CanonicalError::service_unavailable()
                .with_detail("Gateway registry unavailable")
                .create()
        })?;
        let mut entities = Vec::new();
        for schema in registry
            .list_type_schemas(TypeSchemaQuery::default())
            .await?
        {
            entities.push(serde_json::json!({"id":schema.type_uuid,"gts_id":schema.type_id.to_string(),"is_schema":true,"content":schema.raw_schema,"description":schema.description}));
        }
        for instance in registry.list_instances(InstanceQuery::default()).await? {
            let value = serde_json::json!({"id":instance.uuid,"gts_id":instance.id.to_string(),"is_schema":false,"content":instance.object,"description":instance.description});
            if visible(&value, &ctx.subject_tenant_id().to_string()) {
                entities.push(value);
            }
        }
        let count = entities.len();
        return Ok(Json(RegistryResponse { entities, count }));
    }
    let url = target(&source).ok_or_else(|| {
        RegistryError::not_found("Unknown registry source")
            .with_resource(source.clone())
            .create()
    })?;
    let bearer = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            CanonicalError::unauthenticated()
                .with_reason("REGISTRY_AUTH_REQUIRED")
                .create()
        })?;
    let unavailable = || {
        CanonicalError::service_unavailable()
            .with_detail("The selected registry is unavailable")
            .create()
    };
    let response = reader
        .client
        .get(url)
        .header("authorization", bearer)
        .send()
        .await
        .map_err(|_| unavailable())?;
    // Preserve an explicit authentication failure; do not expose an upstream body.
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(CanonicalError::unauthenticated()
            .with_reason("REGISTRY_AUTH_REQUIRED")
            .create());
    }
    if response.status() == StatusCode::FORBIDDEN {
        return Err(RegistryError::permission_denied()
            .with_reason("REGISTRY_READ_DENIED")
            .create());
    }
    let value: Value = response.json().await.map_err(|_| unavailable())?;
    let entries = value["entities"].as_array().ok_or_else(unavailable)?;
    let tenant = ctx.subject_tenant_id().to_string();
    let entities: Vec<_> = entries
        .iter()
        .filter(|entity| visible(entity, &tenant))
        .cloned()
        .collect();
    let count = entities.len();
    Ok(Json(RegistryResponse { entities, count }))
}
pub fn register(router: Router, openapi: &dyn OpenApiRegistry, reader: Arc<Reader>) -> Router {
    OperationBuilder::get("/family-labs/v1/registry/{source}")
        .operation_id("family_labs.registry")
        .summary("Read one allowlisted application registry")
        .tag("Family Labs")
        .authenticated()
        .no_license_required()
        .exposed()
        .path_param("source", "gateway or settings; never a URL")
        .handler(read)
        .json_response_with_schema::<RegistryResponse>(
            openapi,
            StatusCode::OK,
            "Public schemas and own-tenant instances",
        )
        .error_401(openapi)
        .error_403(openapi)
        .error_404(openapi)
        .error_503(openapi)
        .register(router, openapi)
        .layer(Extension(reader))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn source_is_a_closed_allowlist() {
        assert!(target("gateway").is_none()); // Gateway uses its in-process SDK, never the raw HTTP route.
        assert!(target("settings").is_some());
        for value in [
            "http://evil.example",
            "../settings",
            "localhost",
            "settings?url=x",
        ] {
            assert!(target(value).is_none());
        }
    }
    #[test]
    fn diagnostics_does_not_expose_sibling_or_unscoped_instances() {
        assert!(visible(&json!({"is_schema":true,"content":{}}), "dasha"));
        assert!(visible(
            &json!({"is_schema":false,"content":{"tenant_id":"dasha"}}),
            "dasha"
        ));
        assert!(!visible(
            &json!({"is_schema":false,"content":{"tenant_id":"ilya"}}),
            "dasha"
        ));
        assert!(!visible(
            &json!({"is_schema":false,"content":{"token":"private"}}),
            "dasha"
        ));
    }
}
