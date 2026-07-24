#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Integration tests for auth middleware
//!
//! These tests verify that:
//! 1. Auth middleware is properly attached to the router
//! 2. `SecurityContext` is always inserted by middleware
//! 3. Public routes work without authentication
//! 4. Protected routes enforce authentication when enabled

use anyhow::Result;
use async_trait::async_trait;
use authn_resolver_sdk::{
    AuthNResolverClient, AuthNResolverError, AuthenticationResult, ClientCredentialsRequest,
    VerifiedPrincipal,
};
use axum::{
    Extension, Json, Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use serde_json::json;
use std::sync::Arc;
use toolkit::{
    ClientHub, Gear,
    api::{
        OperationBuilder,
        operation_builder::{CORE_GLOBAL_BASE_LICENSE_FEATURE, LicenseFeature},
    },
    config::ConfigProvider,
    context::GearCtx,
    contracts::{ApiGatewayCapability, OpenApiRegistry, RestApiCapability},
};
use toolkit_canonical_errors::Problem;
use toolkit_gts::gts_uri;
use toolkit_security::SecurityContext;
use tower::ServiceExt;
use uuid::Uuid;

const UNAUTHENTICATED_TYPE: &str =
    gts_uri!("cf.core.errors.err.v1~cf.core.err.unauthenticated.v1~");
const SERVICE_UNAVAILABLE_TYPE: &str =
    gts_uri!("cf.core.errors.err.v1~cf.core.err.service_unavailable.v1~");
const INTERNAL_TYPE: &str = gts_uri!("cf.core.errors.err.v1~cf.core.err.internal.v1~");
const PERMISSION_DENIED_TYPE: &str =
    gts_uri!("cf.core.errors.err.v1~cf.core.err.permission_denied.v1~");
const PROBLEM_JSON: &str = "application/problem+json";

async fn problem_from(response: Response) -> Problem {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&body).expect("parse Problem JSON")
}

fn content_type(response: &Response) -> &str {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

/// All `WWW-Authenticate` header values, in order. Empty if none.
///
/// Returning every value (rather than the first) lets assertions also catch a
/// stacked/duplicate challenge, which RFC 7235 permits but the `Bearer` scheme
/// makes ambiguous to parse.
fn www_authenticate_all(response: &Response) -> Vec<&str> {
    response
        .headers()
        .get_all(header::WWW_AUTHENTICATE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect()
}

/// Test configuration provider
struct TestConfigProvider {
    config: serde_json::Value,
}

impl ConfigProvider for TestConfigProvider {
    fn get_gear_config(&self, gear: &str) -> Option<&serde_json::Value> {
        self.config.get(gear)
    }
}

/// Create test context for `api_gateway` gear
fn create_api_gateway_ctx(config: serde_json::Value) -> GearCtx {
    let hub = Arc::new(ClientHub::new());

    GearCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
    )
}

/// Create test context for other test gears
fn create_test_gear_ctx() -> GearCtx {
    GearCtx::new(
        "test_gear",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config: json!({}) }),
        Arc::new(ClientHub::new()),
        tokio_util::sync::CancellationToken::new(),
    )
}

/// Test response type
#[derive(Clone)]
#[toolkit_macros::api_dto(response)]
struct TestResponse {
    message: String,
    user_id: String,
}

/// Handler that requires `SecurityContext` (via Extension extractor)
async fn protected_handler(Extension(ctx): Extension<SecurityContext>) -> Json<TestResponse> {
    Json(TestResponse {
        message: "Protected resource accessed".to_owned(),
        user_id: ctx.subject_id().to_string(),
    })
}

/// Test response exposing `SecurityContext`- and `VerifiedPrincipal`-derived
/// fields independently, so tests can assert both extensions were present
/// and correct without collapsing the comparison into handler logic.
#[derive(Clone)]
#[toolkit_macros::api_dto(response)]
struct PrincipalTestResponse {
    security_subject_id: String,
    principal_subject_id: String,
    external_subject: String,
}

/// Handler that requires both `SecurityContext` and `VerifiedPrincipal`
/// (via Extension extractors). Verifies the gateway inserts the principal on
/// the request when the plugin produced one (ADR 0005). If the principal
/// extension is absent — including when the gateway middleware drops it for
/// violating the `subject_id` invariant — Axum's extractor rejection yields a
/// 5xx, which is the documented behaviour for a principal-requiring route on
/// an authenticated request without one.
async fn principal_handler(
    Extension(ctx): Extension<SecurityContext>,
    Extension(principal): Extension<VerifiedPrincipal>,
) -> Json<PrincipalTestResponse> {
    Json(PrincipalTestResponse {
        security_subject_id: ctx.subject_id().to_string(),
        principal_subject_id: principal.subject_id.to_string(),
        external_subject: principal.external_subject,
    })
}

/// Handler that doesn't require auth
async fn public_handler() -> Json<TestResponse> {
    Json(TestResponse {
        message: "Public resource accessed".to_owned(),
        user_id: "anonymous".to_owned(),
    })
}

/// Test gear with protected and public routes
pub struct TestAuthGear;

#[async_trait]
impl Gear for TestAuthGear {
    async fn init(&self, _ctx: &GearCtx) -> Result<()> {
        Ok(())
    }
}

struct License;

impl AsRef<str> for License {
    fn as_ref(&self) -> &'static str {
        CORE_GLOBAL_BASE_LICENSE_FEATURE
    }
}

impl LicenseFeature for License {}

impl RestApiCapability for TestAuthGear {
    fn register_rest(
        &self,
        _ctx: &GearCtx,
        router: Router,
        openapi: &dyn OpenApiRegistry,
    ) -> Result<Router> {
        // Protected route with explicit auth requirement
        let router = OperationBuilder::get("/tests/v1/api/protected")
            .operation_id("test.protected")
            .authenticated()
            .require_license_features::<License>([])
            .summary("Protected endpoint")
            .handler(protected_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .error_401(openapi)
            .error_403(openapi)
            .register(router, openapi);

        // Protected route with path parameter (to test pattern matching)
        let router = OperationBuilder::get("/tests/v1/api/users/{id}")
            .operation_id("test.get_user")
            .authenticated()
            .require_license_features::<License>([])
            .summary("Get user by ID")
            .path_param("id", "User ID")
            .handler(protected_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .error_401(openapi)
            .error_403(openapi)
            .register(router, openapi);

        // Public route with explicit public marking
        let router = OperationBuilder::get("/tests/v1/api/public")
            .operation_id("test.public")
            .public()
            .summary("Public endpoint")
            .handler(public_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .register(router, openapi);

        Ok(router)
    }
}

#[tokio::test]
async fn test_auth_disabled_mode() {
    // Create api-gateway with auth disabled
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": true,
                "cors_enabled": false,
                "auth_disabled": true,
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    // Register test gear
    let router = Router::new();
    let test_gear = TestAuthGear;
    let router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    // Finalize router (applies middleware)
    let router = api_gateway
        .rest_finalize(&api_ctx, router)
        .expect("Failed to finalize");

    // Test protected route WITHOUT token (should work because auth is disabled)
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Protected route should work when auth is disabled"
    );

    // Test public route
    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/public")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Public route should work"
    );
}

#[tokio::test]
async fn test_public_routes_accessible() {
    // Create api-gateway with auth enabled but test public routes
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": true,
                "cors_enabled": false,
                "auth_disabled": true, // Using disabled for simplicity in test
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    // First call rest_prepare to add built-in routes
    let router = Router::new();
    let router = api_gateway
        .rest_prepare(&api_ctx, router)
        .expect("Failed to prepare");

    // Then register test gear routes
    let test_gear = TestAuthGear;
    let router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    // Finally finalize
    let router = api_gateway
        .rest_finalize(&api_ctx, router)
        .expect("Failed to finalize");

    // Test built-in health endpoints
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Health endpoint should be accessible"
    );

    // Test OpenAPI endpoint
    let response = router
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "OpenAPI endpoint should be accessible"
    );
}

#[tokio::test]
async fn test_public_routes_with_prefix_accessible() {
    // Create api-gateway with auth disabled and test prefixed public routes
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": true,
                "cors_enabled": false,
                "auth_disabled": true, // Using disabled for simplicity in test
                "prefix_path": "/cf",
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    // First call rest_prepare to add built-in routes
    let router = Router::new();
    let router = api_gateway
        .rest_prepare(&api_ctx, router)
        .expect("Failed to prepare");

    // Then register test gear routes
    let test_gear = TestAuthGear;
    let router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    // Finally finalize
    let router = api_gateway
        .rest_finalize(&api_ctx, router)
        .expect("Failed to finalize");

    // Test built-in health endpoints
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Health endpoint should be accessible"
    );

    // Test OpenAPI endpoint
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/cf/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "OpenAPI endpoint should be accessible"
    );

    // Test OpenAPI endpoint
    let response = router
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "OpenAPI endpoint should be inaccessible without prefix"
    );
}

#[tokio::test]
async fn test_middleware_always_inserts_security_ctx() {
    // This test verifies that SecurityContext is always available in handlers
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": false,
                "auth_disabled": true,
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let mut router: Router = Router::new();
    let test_gear = TestAuthGear;
    router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&api_ctx, router)
        .expect("Failed to finalize");

    // Make request to protected handler that extracts SecurityContext
    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    // Should NOT get 500 error about missing SecurityContext
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Handler should receive SecurityContext from middleware"
    );
}

#[tokio::test]
async fn test_openapi_includes_security_metadata() {
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": true,
                "cors_enabled": false,
                "auth_disabled": true,
                "require_auth_by_default": true,
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let router = Router::new();
    let test_gear = TestAuthGear;
    let _router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    // Build OpenAPI spec
    let openapi = api_gateway
        .build_openapi()
        .expect("Failed to build OpenAPI");
    let spec = serde_json::to_value(&openapi).expect("Failed to serialize");

    // Verify security scheme exists
    let security_schemes = spec
        .pointer("/components/securitySchemes")
        .expect("Security schemes should exist");
    assert!(
        security_schemes.get("bearerAuth").is_some(),
        "bearerAuth scheme should be registered"
    );

    // Verify protected route has security requirement
    // Path is /tests/v1/api/protected, JSON pointer escapes / as ~1
    let protected_security = spec.pointer("/paths/~1tests~1v1~1api~1protected/get/security");
    assert!(
        protected_security.is_some(),
        "Protected route should have security requirement in OpenAPI"
    );

    // Verify public route does NOT have security requirement
    let public_security = spec.pointer("/paths/~1tests~1v1~1api~1public/get/security");
    assert!(
        public_security.is_none()
            || public_security
                .unwrap()
                .as_array()
                .is_some_and(Vec::is_empty),
        "Public route should NOT have security requirement in OpenAPI"
    );
}

#[tokio::test]
async fn test_route_pattern_matching_with_path_params() {
    // This test verifies that routes with path parameters (e.g., /users/{id})
    // are properly matched under a configured prefix (auth disabled in this test)
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": false,
                "auth_disabled": true, // Disabled for test simplicity
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let mut router = Router::new();
    let test_gear = TestAuthGear;
    router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&api_ctx, router)
        .expect("Failed to finalize");

    // Test that /tests/v1/api/users/123 is accessible (matches /tests/v1/api/users/{id})
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/users/123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Route with path parameter should be accessible and matched correctly"
    );

    // Test that /tests/v1/api/users/abc-def-456 is also accessible
    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/users/abc-def-456")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Route with different path parameter value should also be accessible"
    );
}

#[tokio::test]
async fn test_route_pattern_matching_with_prefix_path_params() {
    // This test verifies that routes with path parameters (e.g., /users/{id})
    // are properly matched and authorization is enforced
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": false,
                "auth_disabled": true, // Disabled for test simplicity
                "prefix_path": "/cf",
            }
        }
    });

    let api_ctx = create_api_gateway_ctx(config);
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let mut router = Router::new();
    let test_gear = TestAuthGear;
    router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    let router = api_gateway
        .rest_finalize(&api_ctx, router)
        .expect("Failed to finalize");

    // Test that /tests/v1/api/users/123 is accessible (matches /tests/v1/api/users/{id})
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/users/123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Route with path parameter should be accessible and matched correctly"
    );

    // Test that /tests/v1/api/users/abc-def-456 is also accessible
    let response = router
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/users/abc-def-456")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Route with different path parameter value should also be accessible"
    );
}

// ---------------------------------------------------------------------------
// Auth-enabled tests: verify the actual authn_middleware with a mock AuthN client
// ---------------------------------------------------------------------------

/// Handler function type for the mock `AuthN` Resolver.
type MockAuthNHandler =
    dyn Fn(&str) -> Result<AuthenticationResult, AuthNResolverError> + Send + Sync;

/// Configurable mock `AuthN` Resolver client for auth-enabled tests.
struct MockAuthNResolverClient {
    handler: Arc<MockAuthNHandler>,
}

#[async_trait]
impl AuthNResolverClient for MockAuthNResolverClient {
    async fn authenticate(
        &self,
        bearer_token: &str,
    ) -> Result<AuthenticationResult, AuthNResolverError> {
        (self.handler)(bearer_token)
    }

    async fn exchange_client_credentials(
        &self,
        _request: &ClientCredentialsRequest,
    ) -> Result<AuthenticationResult, AuthNResolverError> {
        Err(AuthNResolverError::Internal(
            "not implemented in mock".to_owned(),
        ))
    }
}

/// Test gear for auth-enabled tests.
///
/// Registers both a protected and a public route.
/// The public route also extracts `SecurityContext` so tests can verify
/// that anonymous context is injected for public endpoints.
pub struct TestAuthEnabledGear;

#[async_trait]
impl Gear for TestAuthEnabledGear {
    async fn init(&self, _ctx: &GearCtx) -> Result<()> {
        Ok(())
    }
}

impl RestApiCapability for TestAuthEnabledGear {
    fn register_rest(
        &self,
        _ctx: &GearCtx,
        router: Router,
        openapi: &dyn OpenApiRegistry,
    ) -> Result<Router> {
        let router = OperationBuilder::get("/tests/v1/api/protected")
            .operation_id("test_auth.protected")
            .authenticated()
            .require_license_features::<License>([])
            .summary("Protected endpoint")
            .handler(protected_handler)
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .error_401(openapi)
            .error_403(openapi)
            .register(router, openapi);

        // Protected route that also requires a VerifiedPrincipal (ADR 0005).
        let router = OperationBuilder::get("/tests/v1/api/principal")
            .operation_id("test_auth.principal")
            .authenticated()
            .require_license_features::<License>([])
            .summary("Protected endpoint requiring a verified principal")
            .handler(principal_handler)
            .json_response_with_schema::<PrincipalTestResponse>(
                openapi,
                http::StatusCode::OK,
                "Success",
            )
            .error_401(openapi)
            .error_403(openapi)
            .register(router, openapi);

        // Public route that extracts SecurityContext so tests can verify anonymous ctx
        let router = OperationBuilder::get("/tests/v1/api/public-ctx")
            .operation_id("test_auth.public_ctx")
            .public()
            .summary("Public endpoint with security context")
            .handler(protected_handler) // reuse: extracts SecurityContext
            .json_response_with_schema::<TestResponse>(openapi, http::StatusCode::OK, "Success")
            .register(router, openapi);

        Ok(router)
    }
}

async fn create_router(config: serde_json::Value, mock: MockAuthNResolverClient) -> Router {
    let hub = Arc::new(ClientHub::new());
    hub.register::<dyn AuthNResolverClient>(Arc::new(mock));

    let api_ctx = GearCtx::new(
        "api-gateway",
        Uuid::new_v4(),
        Arc::new(TestConfigProvider { config }),
        hub,
        tokio_util::sync::CancellationToken::new(),
    );
    let test_ctx = create_test_gear_ctx();

    let api_gateway = api_gateway::ApiGateway::default();
    api_gateway.init(&api_ctx).await.expect("Failed to init");

    let mut router = Router::new();
    let test_gear = TestAuthEnabledGear;
    router = test_gear
        .register_rest(&test_ctx, router, &api_gateway)
        .expect("Failed to register routes");

    api_gateway
        .rest_finalize(&api_ctx, router)
        .expect("Failed to finalize")
}

/// Create a finalized router with auth **enabled** and the given mock `AuthN` client.
async fn create_auth_enabled_router(mock: MockAuthNResolverClient, cors_enabled: bool) -> Router {
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": cors_enabled,
                "auth_disabled": false,
            }
        }
    });

    create_router(config, mock).await
}

async fn create_auth_enabled_with_prefix_router(
    mock: MockAuthNResolverClient,
    cors_enabled: bool,
) -> Router {
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                "enable_docs": false,
                "cors_enabled": cors_enabled,
                "auth_disabled": false,
                "prefix_path": "/cf",
            }
        }
    });

    create_router(config, mock).await
}

/// Build a mock that accepts a specific token and returns a `SecurityContext` with known IDs.
fn mock_accepting_token(
    valid_token: &'static str,
    subject_id: Uuid,
    tenant_id: Uuid,
) -> MockAuthNResolverClient {
    MockAuthNResolverClient {
        handler: Arc::new(move |token| {
            if token == valid_token {
                Ok(AuthenticationResult::authenticated(
                    SecurityContext::builder()
                        .subject_id(subject_id)
                        .subject_tenant_id(tenant_id)
                        .build()
                        .unwrap(),
                ))
            } else {
                Err(AuthNResolverError::Unauthorized("invalid token".to_owned()))
            }
        }),
    }
}

/// Build a mock that accepts a specific token and returns a result carrying a
/// `VerifiedPrincipal` (ADR 0005 request-extension tests). The principal's
/// `subject_id` matches the `SecurityContext` subject to uphold the invariant.
fn mock_accepting_token_with_principal(
    valid_token: &'static str,
    subject_id: Uuid,
    tenant_id: Uuid,
    external_subject: &'static str,
) -> MockAuthNResolverClient {
    MockAuthNResolverClient {
        handler: Arc::new(move |token| {
            if token == valid_token {
                let ctx = SecurityContext::builder()
                    .subject_id(subject_id)
                    .subject_tenant_id(tenant_id)
                    .build()
                    .unwrap();
                let principal = VerifiedPrincipal {
                    subject_id,
                    external_subject: external_subject.to_owned(),
                    issuer: "https://issuer.example".to_owned(),
                    email: Some("user@example.com".to_owned()),
                    email_verified: Some(true),
                    provider: "password".to_owned(),
                    is_anonymous: false,
                    issued_at: 0,
                    auth_time: 0,
                    expires_at: 0,
                };
                Ok(AuthenticationResult::with_principal(ctx, principal))
            } else {
                Err(AuthNResolverError::Unauthorized("invalid token".to_owned()))
            }
        }),
    }
}

/// Build a mock that violates the ADR 0005 invariant: the returned
/// `VerifiedPrincipal.subject_id` does NOT match the `SecurityContext`
/// `subject_id` for the same result. Used to test that the gateway
/// middleware drops such a principal instead of inserting it.
fn mock_accepting_token_with_mismatched_principal(
    valid_token: &'static str,
    security_subject_id: Uuid,
    tenant_id: Uuid,
    principal_subject_id: Uuid,
) -> MockAuthNResolverClient {
    MockAuthNResolverClient {
        handler: Arc::new(move |token| {
            if token == valid_token {
                let ctx = SecurityContext::builder()
                    .subject_id(security_subject_id)
                    .subject_tenant_id(tenant_id)
                    .build()
                    .unwrap();
                let principal = VerifiedPrincipal {
                    subject_id: principal_subject_id,
                    external_subject: "mismatched-uid".to_owned(),
                    issuer: "https://issuer.example".to_owned(),
                    email: Some("user@example.com".to_owned()),
                    email_verified: Some(true),
                    provider: "password".to_owned(),
                    is_anonymous: false,
                    issued_at: 0,
                    auth_time: 0,
                    expires_at: 0,
                };
                Ok(AuthenticationResult::with_principal(ctx, principal))
            } else {
                Err(AuthNResolverError::Unauthorized("invalid token".to_owned()))
            }
        }),
    }
}

/// Build a mock that always returns the given error.
fn mock_returning_error(err_fn: fn() -> AuthNResolverError) -> MockAuthNResolverClient {
    MockAuthNResolverClient {
        handler: Arc::new(move |_| Err(err_fn())),
    }
}

/// Build a mock that accepts a specific token and returns a `SecurityContext`
/// carrying the given token scopes (for scope-enforcement tests).
fn mock_accepting_token_with_scopes(
    valid_token: &'static str,
    scopes: Vec<String>,
) -> MockAuthNResolverClient {
    MockAuthNResolverClient {
        handler: Arc::new(move |token| {
            if token == valid_token {
                Ok(AuthenticationResult::authenticated(
                    SecurityContext::builder()
                        .subject_id(Uuid::new_v4())
                        .subject_tenant_id(Uuid::new_v4())
                        .token_scopes(scopes.clone())
                        .build()
                        .unwrap(),
                ))
            } else {
                Err(AuthNResolverError::Unauthorized("invalid token".to_owned()))
            }
        }),
    }
}

/// Create a finalized router with auth **enabled** and route-policy scope
/// enforcement covering `/tests/v1/api/**` with the given required scopes.
async fn create_scope_enforced_router(
    mock: MockAuthNResolverClient,
    required_scopes: &[&str],
) -> Router {
    let config = json!({
        "api-gateway": {
            "config": {
                "bind_addr": "0.0.0.0:8080",
                // Scope enforcement requires auth to be enabled (gear rejects the
                // combination otherwise); stated explicitly as test precondition.
                "auth_disabled": false,
                "route_policies": {
                    "enabled": true,
                    "rules": [
                        { "path": "/tests/v1/api/**", "required_scopes": required_scopes }
                    ]
                }
            }
        }
    });

    create_router(config, mock).await
}

// --- Auth-enabled integration tests ---

#[tokio::test]
async fn test_valid_token_returns_200() {
    let subject_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let mock = mock_accepting_token("valid-test-token", subject_id, tenant_id);

    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["user_id"], subject_id.to_string());
}

#[tokio::test]
async fn test_missing_token_returns_401() {
    let mock = mock_accepting_token("any", Uuid::new_v4(), Uuid::new_v4());
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                // No Authorization header
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Missing token should yield 401"
    );
    assert_eq!(content_type(&response), PROBLEM_JSON);
    // RFC 6750 §3: no credentials were presented, so the challenge carries a
    // `realm` auth-param but no `error` code. Exactly one challenge — no dup.
    assert_eq!(
        www_authenticate_all(&response),
        [r#"Bearer realm="api""#],
        "Missing token should emit a single WWW-Authenticate challenge"
    );
    let problem = problem_from(response).await;
    assert_eq!(problem.problem_type, UNAUTHENTICATED_TYPE);
    assert_eq!(problem.context["reason"], "MISSING_BEARER");
}

#[tokio::test]
async fn test_invalid_token_returns_401() {
    let mock = mock_accepting_token("good-token", Uuid::new_v4(), Uuid::new_v4());
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer bad-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Invalid token should yield 401"
    );
    assert_eq!(content_type(&response), PROBLEM_JSON);
    // RFC 6750 §3.1: a token was presented but rejected, so the challenge
    // carries the `invalid_token` error code. Exactly one challenge — no dup.
    assert_eq!(
        www_authenticate_all(&response),
        [r#"Bearer error="invalid_token""#],
        "Invalid token should emit a single WWW-Authenticate challenge"
    );
    let problem = problem_from(response).await;
    assert_eq!(problem.problem_type, UNAUTHENTICATED_TYPE);
    assert_eq!(problem.context["reason"], "AUTHN_FAILED");
}

#[tokio::test]
async fn test_no_plugin_available_returns_503() {
    let mock = mock_returning_error(|| AuthNResolverError::NoPluginAvailable);
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer some-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "NoPluginAvailable should yield 503"
    );
    assert_eq!(content_type(&response), PROBLEM_JSON);
    // A 503 is an infrastructure failure, not a credential rejection, so it
    // must not carry a bearer challenge.
    assert!(www_authenticate_all(&response).is_empty());
    let problem = problem_from(response).await;
    assert_eq!(problem.problem_type, SERVICE_UNAVAILABLE_TYPE);
    assert_eq!(problem.context["retry_after_seconds"], 5);
}

#[tokio::test]
async fn test_service_unavailable_returns_503() {
    let mock =
        mock_returning_error(|| AuthNResolverError::ServiceUnavailable("plugin down".to_owned()));
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer some-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "ServiceUnavailable should yield 503"
    );
    assert_eq!(content_type(&response), PROBLEM_JSON);
    // A 503 is an infrastructure failure, not a credential rejection, so it
    // must not carry a bearer challenge.
    assert!(www_authenticate_all(&response).is_empty());
    let problem = problem_from(response).await;
    assert_eq!(problem.problem_type, SERVICE_UNAVAILABLE_TYPE);
    assert_eq!(problem.context["retry_after_seconds"], 5);
}

#[tokio::test]
async fn test_internal_error_returns_500() {
    let mock = mock_returning_error(|| AuthNResolverError::Internal("boom".to_owned()));
    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer some-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal error should yield 500"
    );
    assert_eq!(content_type(&response), PROBLEM_JSON);
    // A 500 is an infrastructure failure, not a credential rejection, so it
    // must not carry a bearer challenge.
    assert!(www_authenticate_all(&response).is_empty());
    let problem = problem_from(response).await;
    assert_eq!(problem.problem_type, INTERNAL_TYPE);
}

#[tokio::test]
async fn test_public_route_with_auth_enabled() {
    // Mock that would reject any token — proves it is never called for public routes
    let mock =
        mock_returning_error(|| AuthNResolverError::Internal("should not be called".to_owned()));
    let router = create_auth_enabled_router(mock, false).await;

    // No Authorization header on a public route
    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/public-ctx")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Public route should return 200 even with auth enabled and no token"
    );

    // Verify the handler received an anonymous SecurityContext (subject_id = Uuid::default)
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["user_id"],
        Uuid::default().to_string(),
        "Public route should receive anonymous SecurityContext"
    );
}

#[tokio::test]
async fn test_public_route_with_prefix_auth_enabled() {
    // Mock that would reject any token — proves it is never called for public routes
    let mock =
        mock_returning_error(|| AuthNResolverError::Internal("should not be called".to_owned()));
    let router = create_auth_enabled_with_prefix_router(mock, false).await;

    // No Authorization header on a public route
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/cf/tests/v1/api/public-ctx")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Public route should return 200 even with auth enabled and no token"
    );

    // Verify the handler received an anonymous SecurityContext (subject_id = Uuid::default)
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["user_id"],
        Uuid::default().to_string(),
        "Public route should receive anonymous SecurityContext"
    );

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/public-ctx")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Public route should return 404 for unknown paths"
    );
}

#[tokio::test]
async fn test_cors_preflight_skips_auth() {
    // Mock that rejects everything — proves auth is skipped for preflight
    let mock =
        mock_returning_error(|| AuthNResolverError::Internal("should not be called".to_owned()));
    let router = create_auth_enabled_router(mock, true).await;

    let response = router
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/tests/v1/api/protected")
                .header(header::ORIGIN, "https://example.com")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    // With CORS enabled, a preflight should NOT be rejected by auth.
    // The exact status depends on the CORS layer (usually 200),
    // but it must NOT be 401/403.
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "CORS preflight must not be blocked by auth"
    );
    assert_ne!(
        response.status(),
        StatusCode::FORBIDDEN,
        "CORS preflight must not be blocked by auth"
    );
}

#[tokio::test]
async fn test_insufficient_scope_returns_403_with_challenge() {
    // Valid token, but its scopes do not satisfy the route policy.
    let mock = mock_accepting_token_with_scopes("valid-test-token", vec!["read:other".to_owned()]);
    let router = create_scope_enforced_router(mock, &["admin"]).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Insufficient scope should yield 403"
    );
    assert_eq!(content_type(&response), PROBLEM_JSON);
    // RFC 6750 §3.1: a valid token lacked the required scope, so the challenge
    // carries the `insufficient_scope` error code. Exactly one challenge — no dup.
    assert_eq!(
        www_authenticate_all(&response),
        [r#"Bearer error="insufficient_scope""#],
        "Insufficient scope should emit a single WWW-Authenticate challenge"
    );
    let problem = problem_from(response).await;
    assert_eq!(problem.problem_type, PERMISSION_DENIED_TYPE);
    assert_eq!(problem.context["reason"], "INSUFFICIENT_SCOPES");
}

// --- ADR 0005: VerifiedPrincipal request-extension tests ---

/// When the plugin produces a `VerifiedPrincipal`, the gateway inserts it into
/// the request extensions and it reaches a handler that extracts it —
/// alongside the `SecurityContext`, both independently correct.
#[tokio::test]
async fn test_principal_reaches_handler() {
    let subject_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let mock = mock_accepting_token_with_principal(
        "valid-test-token",
        subject_id,
        tenant_id,
        "firebase-uid-123",
    );

    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/principal")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["external_subject"], "firebase-uid-123");
    assert_eq!(json["principal_subject_id"], subject_id.to_string());
}

/// `SecurityContext` and `VerifiedPrincipal` both reach a handler that
/// extracts both extensions, and their subject ids agree — proving the two
/// extensions genuinely coexist on the request (ADR 0005 invariant), rather
/// than asserting only one of the two extensions ever ran.
#[tokio::test]
async fn test_security_context_and_principal_coexist_with_matching_subject() {
    let subject_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let mock = mock_accepting_token_with_principal(
        "valid-test-token",
        subject_id,
        tenant_id,
        "firebase-uid-123",
    );

    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/principal")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["security_subject_id"], subject_id.to_string());
    assert_eq!(json["principal_subject_id"], subject_id.to_string());
}

/// A principal-requiring route hit with a token whose plugin produced **no**
/// principal returns exactly `500` (Axum's `Extension<T>` extractor
/// rejection), not a 401 — per ADR 0005, a missing principal on an
/// authenticated request is an internal wiring error.
#[tokio::test]
async fn test_missing_principal_on_principal_route_is_500() {
    let subject_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let mock = mock_accepting_token("valid-test-token", subject_id, tenant_id);

    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/principal")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "missing principal on an authenticated principal-route must be exactly 500"
    );
}

/// When a plugin violates the ADR 0005 invariant (`VerifiedPrincipal.subject_id`
/// doesn't match `SecurityContext.subject_id`), the gateway middleware drops
/// the principal instead of inserting it — so a principal-requiring route
/// behaves exactly as if no principal had been produced at all: `500`, not a
/// leaked cross-subject principal.
#[tokio::test]
async fn test_mismatched_principal_is_dropped_not_inserted() {
    let security_subject_id = Uuid::new_v4();
    let principal_subject_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let mock = mock_accepting_token_with_mismatched_principal(
        "valid-test-token",
        security_subject_id,
        tenant_id,
        principal_subject_id,
    );

    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/principal")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "a subject_id-mismatched principal must be dropped, not inserted"
    );
}

/// A bad token on a principal-requiring route still fails with `401` before
/// the request ever reaches the router's extractors — proving auth failure
/// takes precedence over extension/extractor concerns, even for a mock that
/// would have produced a principal had the token been valid. (Principal
/// *insertion* itself can't be observed from outside the middleware; this
/// test instead pins the externally-observable ordering guarantee.)
#[tokio::test]
async fn test_failed_auth_on_principal_route_returns_401_not_500() {
    let mock = mock_accepting_token_with_principal(
        "valid-test-token",
        Uuid::new_v4(),
        Uuid::new_v4(),
        "firebase-uid-123",
    );

    let router = create_auth_enabled_router(mock, false).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/principal")
                .header(header::AUTHORIZATION, "Bearer wrong-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(content_type(&response), PROBLEM_JSON);
    let problem = problem_from(response).await;
    assert_eq!(problem.problem_type, UNAUTHENTICATED_TYPE);
}

#[tokio::test]
async fn test_wildcard_scope_passes_enforcement() {
    // First-party apps carry the `["*"]` scope and bypass route-policy checks.
    let mock = mock_accepting_token_with_scopes("valid-test-token", vec!["*".to_owned()]);
    let router = create_scope_enforced_router(mock, &["admin"]).await;

    let response = router
        .oneshot(
            Request::builder()
                .uri("/tests/v1/api/protected")
                .header(header::AUTHORIZATION, "Bearer valid-test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Wildcard scope should satisfy any route policy"
    );
    // A successful response must not carry a bearer challenge.
    assert!(www_authenticate_all(&response).is_empty());
}
