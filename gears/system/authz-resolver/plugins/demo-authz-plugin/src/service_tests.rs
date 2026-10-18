use super::*;
use parking_lot::Mutex;

struct RecordingHierarchy {
    requests: Mutex<Vec<EvaluationRequest>>,
}

#[async_trait]
impl AuthZResolverPluginClient for RecordingHierarchy {
    async fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> Result<EvaluationResponse, AuthZResolverError> {
        self.requests.lock().push(request);
        Ok(Service::deny())
    }
}

fn fixture() -> (Service, Arc<RecordingHierarchy>) {
    let config: DemoAuthzPluginConfig = serde_json::from_value(serde_json::json!({
        "allowed_tenants": [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)],
        "grants": [
            {"scope": "demo:user", "resource_type": "settings-declaration", "actions": ["read"]},
            {"scope": "demo:user", "resource_type": "tenant-request", "actions": ["consume"]},
            {"scope": "demo:worker", "resource_type": "event-type", "actions": ["produce"], "properties": {"event_type_id": ["settings-changed"]}},
            {"scope": "demo:bootstrap", "resource_type": "*", "actions": ["*"], "subject_ids": [Uuid::from_u128(10)], "subject_tenants": [Uuid::from_u128(1)]}
        ]
    })).unwrap();
    config.validate().unwrap();
    let hierarchy = Arc::new(RecordingHierarchy {
        requests: Mutex::new(Vec::new()),
    });
    (Service::new(config, hierarchy.clone()), hierarchy)
}

fn request(resource: &str, action: &str, scope: &str) -> EvaluationRequest {
    serde_json::from_value(serde_json::json!({
        "subject": {"id": Uuid::from_u128(20), "properties": {"tenant_id": Uuid::from_u128(2)}},
        "action": {"name": action}, "resource": {"type": resource},
        "context": {"token_scopes": [scope]}
    }))
    .unwrap()
}

#[tokio::test]
async fn global_read_has_an_explicit_grant_and_does_not_invent_row_ownership() {
    let (service, hierarchy) = fixture();
    let mut read = request("settings-declaration", "read", "demo:user");
    read.resource.id = Some(Uuid::from_u128(100));
    assert!(service.evaluate(read).await.unwrap().decision);
    assert!(
        !service
            .evaluate(request("settings-declaration", "update", "demo:user"))
            .await
            .unwrap()
            .decision
    );
    assert!(hierarchy.requests.lock().is_empty());
}

#[tokio::test]
async fn concrete_owner_without_resource_id_preserves_a_single_resource_denial() {
    let (service, hierarchy) = fixture();
    let mut input = request("tenant-request", "consume", "demo:user");
    input.resource.properties.insert(
        OWNER_TENANT_ID.to_owned(),
        serde_json::json!(Uuid::from_u128(3)),
    );
    input
        .context
        .supported_properties
        .push(OWNER_TENANT_ID.to_owned());
    assert!(!service.evaluate(input).await.unwrap().decision);
    let delegated = hierarchy.requests.lock();
    assert_eq!(delegated.len(), 1);
    assert_eq!(delegated[0].resource.id, Some(Uuid::from_u128(3)));
}

#[tokio::test]
async fn explicit_tenant_selector_on_global_resource_also_requires_hierarchy_check() {
    let (service, hierarchy) = fixture();
    let mut input = request("settings-declaration", "read", "demo:user");
    input.resource.id = Some(Uuid::from_u128(100));
    input.context.tenant_context = Some(authz_resolver_sdk::TenantContext {
        root_id: Some(Uuid::from_u128(3)),
        ..Default::default()
    });
    assert!(!service.evaluate(input).await.unwrap().decision);
    assert_eq!(
        hierarchy.requests.lock()[0].resource.id,
        Some(Uuid::from_u128(3))
    );
}

#[tokio::test]
async fn worker_can_produce_only_the_explicit_demo_type() {
    let (service, _) = fixture();
    let mut publish = request("event-type", "produce", "demo:worker");
    assert!(!service.evaluate(publish.clone()).await.unwrap().decision);
    publish
        .resource
        .properties
        .insert("event_type_id".to_owned(), serde_json::json!("other-event"));
    assert!(!service.evaluate(publish.clone()).await.unwrap().decision);
    publish.resource.properties.insert(
        "event_type_id".to_owned(),
        serde_json::json!("settings-changed"),
    );
    assert!(service.evaluate(publish).await.unwrap().decision);
}

#[tokio::test]
async fn bootstrap_scope_is_bound_to_its_subject_and_home_tenant() {
    let (service, _) = fixture();
    let mut input = request("unlisted-resource", "update", "demo:bootstrap");
    assert!(!service.evaluate(input.clone()).await.unwrap().decision);
    input.subject.id = Uuid::from_u128(10);
    assert!(!service.evaluate(input.clone()).await.unwrap().decision);
    input.subject.properties.insert(
        "tenant_id".to_owned(),
        serde_json::json!(Uuid::from_u128(1)),
    );
    assert!(service.evaluate(input).await.unwrap().decision);
}

#[tokio::test]
async fn unknown_tenant_and_missing_scopes_are_denied_before_delegation() {
    let (service, hierarchy) = fixture();
    let mut input = request("tenant-request", "consume", "demo:user");
    input.resource.properties.insert(
        OWNER_TENANT_ID.to_owned(),
        serde_json::json!(Uuid::from_u128(999)),
    );
    assert!(!service.evaluate(input).await.unwrap().decision);
    let mut input = request("settings-declaration", "read", "demo:user");
    input.context.token_scopes.clear();
    assert!(!service.evaluate(input).await.unwrap().decision);
    assert!(hierarchy.requests.lock().is_empty());
}
