//! Credential dispatch is a choice before validation, never a fallback after failure.
use async_trait::async_trait;
use authn_resolver_sdk::{
    AuthNResolverError, AuthNResolverPluginClient, AuthenticationResult, ClientCredentialsRequest,
};
use std::sync::Arc;

pub struct Service {
    firebase: Arc<dyn AuthNResolverPluginClient>,
    technical: Arc<dyn AuthNResolverPluginClient>,
}
impl Service {
    pub fn new(
        firebase: Arc<dyn AuthNResolverPluginClient>,
        technical: Arc<dyn AuthNResolverPluginClient>,
    ) -> Self {
        Self {
            firebase,
            technical,
        }
    }
}
#[async_trait]
impl AuthNResolverPluginClient for Service {
    async fn authenticate(&self, token: &str) -> Result<AuthenticationResult, AuthNResolverError> {
        if token.is_empty() || token.len() > 8192 {
            return Err(AuthNResolverError::Unauthorized(
                "invalid credential".to_owned(),
            ));
        }
        // Demo technical bearer tokens are opaque random values. A JWT, including
        // a malformed/expired one, is verified exclusively by Firebase.
        if token.contains('.') {
            self.firebase.authenticate(token).await
        } else {
            self.technical.authenticate(token).await
        }
    }
    async fn exchange_client_credentials(
        &self,
        request: &ClientCredentialsRequest,
    ) -> Result<AuthenticationResult, AuthNResolverError> {
        self.technical.exchange_client_credentials(request).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "strict credential fixtures")]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;
    struct RejectingFirebase(AtomicUsize);
    #[async_trait]
    impl AuthNResolverPluginClient for RejectingFirebase {
        async fn authenticate(&self, _: &str) -> Result<AuthenticationResult, AuthNResolverError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(AuthNResolverError::Unauthorized(
                "invalid Firebase credential".to_owned(),
            ))
        }
        async fn exchange_client_credentials(
            &self,
            _: &ClientCredentialsRequest,
        ) -> Result<AuthenticationResult, AuthNResolverError> {
            panic!("client credentials must never reach Firebase")
        }
    }
    fn fixture() -> (Service, Arc<RejectingFirebase>) {
        let identity = serde_json::json!({"subject_id": Uuid::from_u128(10), "subject_tenant_id": Uuid::from_u128(1), "token_scopes": ["demo:worker"]});
        let config: static_authn_plugin::config::StaticAuthNPluginConfig = serde_json::from_value(serde_json::json!({
            "mode": "static_tokens", "tokens": [{"token": "technical-opaque-token", "identity": identity}, {"token": "known.jwt.token", "identity": identity}],
            "s2s_credentials": [{"client_id": "worker", "client_secret": "configured-secret", "identity": identity}]
        })).unwrap();
        let technical = Arc::new(static_authn_plugin::domain::Service::from_config(&config));
        let firebase = Arc::new(RejectingFirebase(AtomicUsize::new(0)));
        (Service::new(firebase.clone(), technical), firebase)
    }
    #[tokio::test]
    async fn opaque_tokens_use_the_existing_strict_static_implementation() {
        let (service, firebase) = fixture();
        let result = service
            .authenticate("technical-opaque-token")
            .await
            .unwrap();
        assert_eq!(result.security_context.subject_id(), Uuid::from_u128(10));
        assert!(matches!(
            service.authenticate("unknown-opaque-token").await,
            Err(AuthNResolverError::Unauthorized(_))
        ));
        assert_eq!(firebase.0.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn a_rejected_jwt_never_falls_back_even_if_static_auth_would_accept_it() {
        let (service, firebase) = fixture();
        assert!(matches!(
            service.authenticate("known.jwt.token").await,
            Err(AuthNResolverError::Unauthorized(_))
        ));
        assert_eq!(firebase.0.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn client_credentials_are_delegated_to_the_existing_s2s_implementation() {
        let (service, firebase) = fixture();
        let request = ClientCredentialsRequest {
            client_id: "worker".to_owned(),
            client_secret: secrecy::SecretString::from("configured-secret"),
            scopes: vec![],
        };
        assert_eq!(
            service
                .exchange_client_credentials(&request)
                .await
                .unwrap()
                .security_context
                .subject_id(),
            Uuid::from_u128(10)
        );
        let bad = ClientCredentialsRequest {
            client_secret: secrecy::SecretString::from("wrong-secret"),
            ..request
        };
        assert!(service.exchange_client_credentials(&bad).await.is_err());
        assert_eq!(firebase.0.load(Ordering::SeqCst), 0);
    }
}
