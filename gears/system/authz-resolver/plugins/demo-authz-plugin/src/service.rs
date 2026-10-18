//! Action grants and concrete target-tenant checks are independent requirements.
use crate::config::{DemoAuthzPluginConfig, Grant};
use async_trait::async_trait;
use authz_resolver_sdk::{
    AuthZResolverError, AuthZResolverPluginClient, EvaluationRequest, EvaluationResponse,
    EvaluationResponseContext,
};
use std::sync::Arc;
use toolkit_security::pep_properties::OWNER_TENANT_ID;
use uuid::Uuid;

pub struct Service {
    config: DemoAuthzPluginConfig,
    hierarchy: Arc<dyn AuthZResolverPluginClient>,
}
impl Service {
    pub fn new(
        config: DemoAuthzPluginConfig,
        hierarchy: Arc<dyn AuthZResolverPluginClient>,
    ) -> Self {
        Self { config, hierarchy }
    }
    fn deny() -> EvaluationResponse {
        EvaluationResponse {
            decision: false,
            context: EvaluationResponseContext::default(),
        }
    }
    fn permits(grant: &Grant, request: &EvaluationRequest, tenant: Uuid) -> bool {
        request.context.token_scopes.contains(&grant.scope)
            && (grant.resource_type == "*" || grant.resource_type == request.resource.resource_type)
            && (grant.actions.contains("*") || grant.actions.contains(&request.action.name))
            && (grant.subject_ids.is_empty() || grant.subject_ids.contains(&request.subject.id))
            && (grant.subject_tenants.is_empty() || grant.subject_tenants.contains(&tenant))
            && grant.properties.iter().all(|(property, values)| {
                request
                    .resource
                    .properties
                    .get(property)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| values.contains(value))
            })
    }
}
#[async_trait]
impl AuthZResolverPluginClient for Service {
    async fn evaluate(
        &self,
        mut request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        let tenant = request
            .subject
            .properties
            .get("tenant_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok());
        let Some(tenant) = tenant.filter(|id| self.config.allowed_tenants.contains(id)) else {
            return Ok(Self::deny());
        };
        if !self
            .config
            .grants
            .iter()
            .any(|grant| Self::permits(grant, &request, tenant))
        {
            return Ok(Self::deny());
        }
        let has_owner = request.resource.properties.contains_key(OWNER_TENANT_ID);
        let owner = request
            .resource
            .properties
            .get(OWNER_TENANT_ID)
            .and_then(serde_json::Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok());
        if has_owner && owner.is_none_or(|id| !self.config.allowed_tenants.contains(&id)) {
            return Ok(Self::deny());
        }
        let has_row_properties = !request.context.supported_properties.is_empty();
        let has_groups = request.resource.properties.contains_key("group_ids")
            || request
                .resource
                .properties
                .contains_key("ancestor_group_ids");
        let selected_root = request
            .context
            .tenant_context
            .as_ref()
            .and_then(|context| context.root_id);
        if selected_root.is_some_and(|id| !self.config.allowed_tenants.contains(&id)) {
            return Ok(Self::deny());
        }
        if !has_owner && !has_row_properties && !has_groups && selected_root.is_none() {
            // Declarations and schemas are platform-global, rather than rows
            // whose owner is inferred from an arbitrary resource UUID.
            return Ok(EvaluationResponse {
                decision: true,
                context: EvaluationResponseContext::default(),
            });
        }
        if let Some(owner) = owner {
            // EB tenant-scope checks carry a concrete owner without a resource
            // ID. Force TR's single-resource branch; an unused list scope is
            // not proof that this requested tenant is authorized.
            request.resource.id = Some(owner);
        } else if !has_row_properties
            && !has_groups
            && let Some(root) = selected_root
        {
            // A global resource's UUID does not identify a tenant. Validate an
            // explicit tenant selector against its concrete root instead.
            request.resource.id = Some(root);
            request.resource.properties.insert(
                OWNER_TENANT_ID.to_owned(),
                serde_json::Value::String(root.to_string()),
            );
        }
        let mut response = self.hierarchy.evaluate(request).await?;
        if response.decision && !has_row_properties && !has_groups {
            response.context.constraints.clear();
        }
        Ok(response)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "policy translation fixtures")]
#[path = "service_tests.rs"]
mod tests;
