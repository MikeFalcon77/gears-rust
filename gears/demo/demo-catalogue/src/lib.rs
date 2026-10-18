//! Demo-owned catalogue restored through the SDK in each process, before serving.
use async_trait::async_trait;
use event_broker_sdk as _;
use serde_json::json;
use toolkit::{Gear, GearCtx};
use types_registry_sdk::{RegisterResult, TypesRegistryClient};

pub const TOPIC: &str = "gts.cf.core.events.topic.v1~cf.demo.settings.changed.v1";
pub const EVENT_TYPE: &str = "gts.cf.core.events.event.v1~cf.demo.settings.changed.v1~";
pub const SUBJECT_TYPE: &str = "gts.cf.demo.settings.setting.v1~";

#[toolkit::gear(name = "demo-catalogue", deps = [types_registry])]
#[derive(Default)]
pub struct DemoCatalogue;

#[async_trait]
impl Gear for DemoCatalogue {
    async fn init(&self, ctx: &GearCtx) -> anyhow::Result<()> {
        let registry = ctx.client_hub().get::<dyn TypesRegistryClient>()?;
        // Parent event schemas are linked from the EB SDK's inventory. These
        // demo declarations are restored after each V1 in-memory restart.
        let entities = vec![
            json!({"$id": format!("gts://{SUBJECT_TYPE}"), "$schema": "http://json-schema.org/draft-07/schema#", "type": "object"}),
            json!({"id": TOPIC, "description": "Settings changes in Family Labs"}),
            json!({"$id": format!("gts://{EVENT_TYPE}"), "$schema": "http://json-schema.org/draft-07/schema#", "type": "object",
                "x-gts-traits": {"topic": TOPIC, "allowed_subject_types": [SUBJECT_TYPE], "partition_key": "/tenant_id"},
                "allOf": [{"$ref": "gts://gts.cf.core.events.event.v1~"},
                    {"type": "object", "properties": {"data": {"type": ["object", "null"]}}}]}),
        ];
        RegisterResult::ensure_all_ok(&registry.register(entities).await?)?;
        Ok(())
    }
}
