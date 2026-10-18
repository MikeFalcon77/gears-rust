//! Authorized, non-secret effective reads across a process boundary.
use crate::{
    SettingKey,
    models::{EffectiveValueResponse, GetEffectiveRequest},
};
use serde::{Deserialize, Serialize};
use toolkit_canonical_errors::{CanonicalError, Problem};
use toolkit_security::SecurityContext;

/// A bounded explicit set; category enumeration remains an administrative API.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkEffectiveRequest {
    /// At most 500 keys, each independently resolved.
    pub keys: Vec<SettingKey>,
    /// The target scope, checked against the authenticated caller's hierarchy.
    pub scope: String,
}

/// A key's success or canonical failure. Request-level denial stays an error.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RemoteBulkOutcome {
    /// Effective public value.
    Value {
        key: SettingKey,
        value: EffectiveValueResponse,
    },
    /// No fallback to a schema default on a failed read.
    Error {
        key: SettingKey,
        #[schemars(with = "serde_json::Value")]
        problem: Problem,
    },
}

/// Tenant-plane reads; the server authorizes the original user on every call.
#[toolkit::contract(gear = "settings-service", version = "v1")]
pub trait SettingsReadApi: Send + Sync {
    /// Read one public setting. Secret and PII declarations are not exposed.
    #[idempotency(SafeRead)]
    async fn get_effective(
        &self,
        ctx: SecurityContext,
        req: GetEffectiveRequest,
    ) -> Result<EffectiveValueResponse, CanonicalError>;
    /// Read a bounded list with independent per-key outcomes.
    #[idempotency(SafeRead)]
    async fn get_effective_bulk(
        &self,
        ctx: SecurityContext,
        req: BulkEffectiveRequest,
    ) -> Result<Vec<RemoteBulkOutcome>, CanonicalError>;
}

impl toolkit::api::api_dto::RequestApiDto for BulkEffectiveRequest {}
impl toolkit::api::api_dto::ResponseApiDto for RemoteBulkOutcome {}
