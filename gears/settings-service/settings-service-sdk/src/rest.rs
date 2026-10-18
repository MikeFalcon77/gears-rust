//! REST projection. Bearer identity is forwarded; no platform-plane shortcut.
use crate::{
    SettingsReadApi,
    models::{EffectiveValueResponse, GetEffectiveRequest},
    remote::{BulkEffectiveRequest, RemoteBulkOutcome},
};
use toolkit_canonical_errors::CanonicalError;
use toolkit_security::SecurityContext;

/// Internal contract routes consumed directly by in-cluster adapters.
#[toolkit::rest_contract(base_path = "/settings-service/v1")]
pub trait SettingsReadApiRest: SettingsReadApi {
    /// Safe read with a typed request body.
    #[post("/effective:read")]
    async fn get_effective(
        &self,
        ctx: SecurityContext,
        req: GetEffectiveRequest,
    ) -> Result<EffectiveValueResponse, CanonicalError>;
    /// Bounded bulk read.
    #[post("/effective:readBulk")]
    async fn get_effective_bulk(
        &self,
        ctx: SecurityContext,
        req: BulkEffectiveRequest,
    ) -> Result<Vec<RemoteBulkOutcome>, CanonicalError>;
}
