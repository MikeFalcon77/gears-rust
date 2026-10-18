//! Optional community integration using the existing transactional producer.
//! Payloads contain invalidation coordinates only, never setting values or PII.
use super::value_writes::WriteCoordinator;
use authn_resolver_sdk::AuthNResolverClient;
use event_broker_sdk::{
    api::EventBrokerApi,
    producer::{
        DbDeduplication, DbProducer, MissingProducerRegistration, ProducerIdentity, ProducerMode,
        ProducerOutboxHandle,
    },
    rest::RestBroker,
    typed_event::TypedEvent,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, sync::Arc};
use toolkit::client_hub::ClientHub;
use toolkit_db::{
    DBProvider, DbError,
    outbox::{Outbox, Partitions},
};
use uuid::Uuid;

const TOPIC: &str = "gts.cf.core.events.topic.v1~cf.demo.settings.changed.v1";
const EVENT_TYPE: &str = "gts.cf.core.events.event.v1~cf.demo.settings.changed.v1~";
const TABLE_PREFIX: &str = "settings_events";

/// Deployment-owned; the credential itself is read from a Secret environment.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityEventsConfig {
    pub broker_url: String,
    pub bearer_env: String,
    /// Limits this demo publisher to the explicitly configured child tenants.
    pub tenants: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsChanged {
    pub key: String,
    pub tenant_id: Uuid,
    pub change_set_id: Uuid,
    pub request_id: String,
}
impl TypedEvent for SettingsChanged {
    const TYPE_ID: &'static str = EVENT_TYPE;
    const SUBJECT_TYPE: &'static str = "gts.cf.demo.settings.setting.v1~";
    const SOURCE: &'static str = "settings-service";
    fn subject(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.key)
    }
    fn tenant_id(&self) -> Option<Uuid> {
        Some(self.tenant_id)
    }
}

/// Shared outbox and managed-producer tables for the Settings database.
///
/// # Panics
/// Only if the source-controlled SQL identifier ceases to be a valid prefix.
#[must_use]
#[allow(
    clippy::expect_used,
    reason = "source-controlled SQL identifier is a static invariant"
)]
pub fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
    // Literal prefix is owned here and shared with the builder below.
    let mut migrations = toolkit_db::outbox::outbox_migrations_with_prefix(TABLE_PREFIX)
        .expect("static valid outbox prefix");
    migrations.extend(event_broker_sdk::producer::producer_registration_migrations());
    migrations
}

/// Prepare schemas and producer registration outside business transactions.
///
/// # Errors
/// Missing or rejected worker identity, unavailable broker, or database setup.
pub async fn start(
    config: &CommunityEventsConfig,
    db: &DBProvider<DbError>,
    hub: &ClientHub,
    writes: &WriteCoordinator,
) -> anyhow::Result<ProducerOutboxHandle> {
    anyhow::ensure!(
        !config.tenants.is_empty() && config.tenants.iter().all(|id| !id.is_nil()),
        "community event tenants must be explicit non-nil IDs"
    );
    let token = std::env::var(&config.bearer_env).map_err(|_| {
        anyhow::anyhow!("community event credential environment variable is missing")
    })?;
    anyhow::ensure!(!token.is_empty(), "community event credential is empty");
    let authn = hub.get::<dyn AuthNResolverClient>()?;
    let identity = authn.authenticate(&token).await?;
    let ctx = identity.security_context;
    anyhow::ensure!(
        ctx.has_scope("demo:worker") && ctx.bearer_token().is_some(),
        "authenticated worker scope and forwarding credential required"
    );
    let broker: Arc<dyn EventBrokerApi> = Arc::new(RestBroker::new(&config.broker_url)?);
    let warning = toolkit::telemetry::ThrottledLog::new(std::time::Duration::from_secs(30));
    let producer = loop {
        let result = DbProducer::builder()
            .broker(Arc::clone(&broker))
            .db(db.db())
            .security_context(ctx.clone())
            .identity(
                ProducerIdentity::new()
                    .source("settings-service")
                    .client_agent("family-labs-settings/1.0"),
            )
            .deduplication(
                DbDeduplication::managed(ProducerMode::Monotonic)
                    .key("family-labs-settings")
                    .on_missing(MissingProducerRegistration::RegisterNew),
            )
            .topics([TOPIC])
            .event_type_patterns([EVENT_TYPE])
            .prepare_all()
            .await;
        match result {
            Ok(producer) => break producer,
            Err(
                err @ (event_broker_sdk::EventBrokerError::Transport(_)
                | event_broker_sdk::EventBrokerError::EventTypeUnknown { .. }
                | event_broker_sdk::EventBrokerError::TopicNotFound { .. }),
            ) => {
                if warning.should_log() {
                    tracing::warn!(error = %err, "Waiting for the remote event broker before Settings readiness");
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            Err(err) => return Err(err.into()),
        }
    };
    let queue = producer.outbox_queue("settings-changes", Partitions::of(1))?;
    let handle = queue
        .start(Outbox::builder(db.db()).table_prefix(TABLE_PREFIX)?)
        .await?;
    writes.install_event_outbox(handle.outbox().clone())?;
    Ok(handle)
}
