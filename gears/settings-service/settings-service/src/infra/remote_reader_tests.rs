use super::*;
use crate::test_support::{ResolutionHarness, SECRET, TEXT, context_for};
use serde_json::json;
use toolkit_security::PlatformSecurityContext;

struct Decision(bool);
#[async_trait]
impl authz_resolver_sdk::AuthZResolverApi for Decision {
    async fn evaluate(
        &self,
        _: PlatformSecurityContext,
        _: authz_resolver_sdk::EvaluationRequest,
    ) -> Result<authz_resolver_sdk::EvaluationResponse, CanonicalError> {
        Ok(authz_resolver_sdk::EvaluationResponse {
            decision: self.0,
            context: Default::default(),
        })
    }
}
fn reader(h: &ResolutionHarness, allowed: bool) -> RemoteReader {
    RemoteReader {
        db: Arc::clone(&h.db),
        resolver: Arc::clone(&h.resolver),
        enforcer: Arc::new(PolicyEnforcer::new(Arc::new(Decision(allowed)))),
    }
}
fn status(error: CanonicalError) -> u16 {
    let problem: toolkit_canonical_errors::Problem = error.into();
    problem.status.expect("canonical HTTP status")
}
#[tokio::test]
async fn reads_own_public_value_but_denies_sibling_ancestor_and_standalone() {
    let h = ResolutionHarness::new().await;
    h.declare("flag", "local", json!(false)).await;
    let reader = reader(&h, true);
    let ctx = context_for(h.tree.a);
    let req = |scope| GetEffectiveRequest {
        key: h.key("flag"),
        scope,
    };
    let own = reader
        .get_effective(ctx.clone(), req(format!("/tenants/{}", h.tree.a)))
        .await
        .expect("own value");
    assert_eq!(own.value, json!(false));
    for target in [h.tree.c, h.tree.root, h.tree.s] {
        let error = reader
            .get_effective(ctx.clone(), req(format!("/tenants/{target}")))
            .await
            .expect_err("outside scope");
        assert_eq!(status(error), 403);
    }
    assert_eq!(
        status(
            reader
                .get_effective(ctx, req("/".into()))
                .await
                .expect_err("platform scope")
        ),
        403
    );
}
#[tokio::test]
async fn remote_contract_hides_secret_and_pii_values() {
    let h = ResolutionHarness::new().await;
    h.declare_typed("credential", "local", json!(""), SECRET, "secret")
        .await;
    h.declare_typed("personal", "local", json!("private"), TEXT, "pii")
        .await;
    let reader = reader(&h, true);
    for name in ["credential", "personal"] {
        let req = GetEffectiveRequest {
            key: h.key(name),
            scope: format!("/tenants/{}", h.tree.a),
        };
        assert_eq!(
            status(
                reader
                    .get_effective(context_for(h.tree.a), req)
                    .await
                    .expect_err("not public")
            ),
            404
        );
    }
}
#[tokio::test]
async fn bulk_gates_even_empty_requests_and_keeps_independent_key_failures() {
    let h = ResolutionHarness::new().await;
    h.declare("flag", "local", json!(false)).await;
    let ctx = context_for(h.tree.a);
    let scope = format!("/tenants/{}", h.tree.a);
    let req = BulkEffectiveRequest {
        keys: vec![],
        scope: scope.clone(),
    };
    assert_eq!(
        status(
            reader(&h, false)
                .get_effective_bulk(ctx.clone(), req)
                .await
                .expect_err("PDP denial")
        ),
        403
    );
    let req = BulkEffectiveRequest {
        keys: vec![],
        scope: format!("/tenants/{}", h.tree.c),
    };
    assert_eq!(
        status(
            reader(&h, true)
                .get_effective_bulk(ctx.clone(), req)
                .await
                .expect_err("sibling")
        ),
        403
    );
    let req = BulkEffectiveRequest {
        keys: vec![h.key("flag"), h.key("missing")],
        scope: scope.clone(),
    };
    let results = reader(&h, true)
        .get_effective_bulk(ctx.clone(), req)
        .await
        .expect("bulk");
    assert!(matches!(&results[0], RemoteBulkOutcome::Value { .. }));
    assert!(
        matches!(&results[1], RemoteBulkOutcome::Error { problem, .. } if problem.status == Some(404))
    );
    let req = BulkEffectiveRequest {
        keys: vec![h.key("flag"); 501],
        scope,
    };
    assert_eq!(
        status(
            reader(&h, true)
                .get_effective_bulk(ctx, req)
                .await
                .expect_err("bound")
        ),
        400
    );
}
