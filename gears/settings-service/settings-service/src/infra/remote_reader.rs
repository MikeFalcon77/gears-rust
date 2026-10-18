//! PEP for the remote contract. The trusted local reader remains separate.
use crate::{
    api::{
        authz::{self, resource},
        rest::setting_handlers::gate_target,
    },
    domain::{category::visibility, error::DomainError, resolution::ScopeTarget},
    gear::ConcreteResolver,
};
use async_trait::async_trait;
use authz_resolver_sdk::PolicyEnforcer;
use settings_service_sdk::{
    SettingsReadApi,
    models::{EffectiveValueResponse, GetEffectiveRequest},
    remote::{BulkEffectiveRequest, RemoteBulkOutcome},
};
use std::sync::Arc;
use toolkit_canonical_errors::CanonicalError;
use toolkit_db::{DBProvider, DbError};
use toolkit_security::SecurityContext;

/// The authorized projection shares the resolver/cache with administrative reads.
pub struct RemoteReader {
    pub(crate) db: Arc<DBProvider<DbError>>,
    pub(crate) resolver: Arc<ConcreteResolver>,
    pub(crate) enforcer: Arc<PolicyEnforcer>,
}
impl RemoteReader {
    async fn target(
        &self,
        ctx: &SecurityContext,
        scope: &str,
    ) -> Result<ScopeTarget, CanonicalError> {
        let root = self.resolver.root_tenant().await?;
        let requested = ScopeTarget::parse(scope)?.tenant_id(root);
        Ok(gate_target(&self.resolver, ctx, Some(requested)).await?)
    }
}
#[async_trait]
impl SettingsReadApi for RemoteReader {
    async fn get_effective(
        &self,
        ctx: SecurityContext,
        req: GetEffectiveRequest,
    ) -> Result<EffectiveValueResponse, CanonicalError> {
        let scope =
            authz::access_scope(&self.enforcer, &ctx, &resource::VALUE, "read", None).await?;
        let target = self.target(&ctx, &req.scope).await?;
        let conn = self.db.conn().map_err(|err| DomainError::Internal {
            diagnostic: err.to_string(),
        })?;
        let effective = self.resolver.resolve(&conn, &req.key, target).await?;
        let caller = ScopeTarget::Tenant(ctx.subject_tenant_id())
            .normalize(self.resolver.root_tenant().await?);
        if effective.data_classification != "public"
            || self
                .resolver
                .effective_access(&conn, effective.declaration_id, caller)
                .await?
                .is_hidden()
            || !visibility::is_visible(
                &visibility::domain_visibility(&scope),
                effective.domain_affinity.as_deref(),
            )
        {
            return Err(DomainError::NotFound {
                resource: "declaration",
            }
            .into());
        }
        super::reader_client::project(req.scope, &effective)
    }
    async fn get_effective_bulk(
        &self,
        ctx: SecurityContext,
        req: BulkEffectiveRequest,
    ) -> Result<Vec<RemoteBulkOutcome>, CanonicalError> {
        // Request-level checks apply even to an empty list and precede per-key work.
        authz::access_scope(&self.enforcer, &ctx, &resource::VALUE, "read", None).await?;
        self.target(&ctx, &req.scope).await?;
        if req.keys.len() > settings_service_sdk::api::BULK_LIMIT {
            return Err(DomainError::Validation {
                field: "keys".into(),
                code: crate::field::BULK_TOO_LARGE,
                message: "At most 500 setting keys are allowed.".into(),
            }
            .into());
        }
        let mut results = Vec::with_capacity(req.keys.len());
        for key in req.keys {
            let request = GetEffectiveRequest {
                key: key.clone(),
                scope: req.scope.clone(),
            };
            results.push(match self.get_effective(ctx.clone(), request).await {
                Ok(value) => RemoteBulkOutcome::Value { key, value },
                Err(err) => RemoteBulkOutcome::Error {
                    key,
                    problem: err.into(),
                },
            });
        }
        Ok(results)
    }
}

#[cfg(test)]
#[path = "remote_reader_tests.rs"]
mod tests;
