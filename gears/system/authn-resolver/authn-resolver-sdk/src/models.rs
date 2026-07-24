//! Domain models for the `AuthN` resolver gear.

use secrecy::SecretString;
use uuid::Uuid;

use toolkit_security::SecurityContext;

/// Result of a successful authentication.
///
/// Contains the validated `SecurityContext` with identity information
/// populated from the token (`subject_id`, `subject_tenant_id`, `token_scopes`, etc.),
/// and an optional [`VerifiedPrincipal`] carrying verified profile/time claims for
/// handlers that need them (see [ADR 0005]).
///
/// [ADR 0005]: https://github.com/constructorfabric/gears-rust/blob/main/docs/arch/authorization/ADR/0005-verified-principal-request-extension.md
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// The validated security context with identity fields populated.
    ///
    /// Contains:
    /// - `subject_id` — The authenticated user/service ID
    /// - `subject_tenant_id` — The subject's home tenant
    /// - `token_scopes` — Token capability restrictions
    /// - `bearer_token` — Original token for PDP forwarding
    /// - `tenant_id` — Context tenant (may be set by `AuthN` or later by middleware)
    pub security_context: SecurityContext,

    /// Optional verified profile claims for the same token that produced
    /// `security_context`.
    ///
    /// Populated only by plugins that expose verified profile/time claims
    /// (e.g. an OIDC/Firebase plugin serving a `/me`-style endpoint). Most
    /// plugins leave this `None`. When `Some`, the gateway inserts it into the
    /// request extensions alongside `SecurityContext`. `SecurityContext` stays
    /// PDP-minimal; profile/time claims live here instead.
    pub principal: Option<VerifiedPrincipal>,
}

impl AuthenticationResult {
    /// Construct a result carrying only the validated `SecurityContext`
    /// (no verified principal). This is the common case for plugins that do
    /// not expose profile claims.
    #[must_use]
    pub fn authenticated(security_context: SecurityContext) -> Self {
        Self {
            security_context,
            principal: None,
        }
    }

    /// Construct a result carrying both the validated `SecurityContext` and a
    /// [`VerifiedPrincipal`] with verified profile/time claims.
    ///
    /// The caller must uphold the invariant that
    /// `principal.subject_id == security_context.subject_id()`.
    #[must_use]
    pub fn with_principal(security_context: SecurityContext, principal: VerifiedPrincipal) -> Self {
        Self {
            security_context,
            principal: Some(principal),
        }
    }
}

/// Verified profile and token-time claims for an authenticated caller.
///
/// Provider-agnostic shape produced by an `AuthN` plugin from the **verified**
/// token, carried on the request that produced it (via the request extensions)
/// so handlers read claims scoped to the current token rather than a
/// process-local cache. See [ADR 0005].
///
/// Kept separate from `SecurityContext` so the PDP identity stays minimal:
/// `SecurityContext` is the authorization identity; `VerifiedPrincipal` is
/// `AuthN` output for handlers that need profile/time claims.
///
/// **Invariant:** when carried on an [`AuthenticationResult`], `subject_id`
/// equals the `SecurityContext`'s `subject_id`.
///
/// `email` and `external_subject` are PII, not credentials: unlike
/// `SecurityContext::bearer_token`, they are meant to be serialized into
/// client-facing responses (e.g. a `/me` endpoint), so they stay plain
/// `String`/`Option<String>` rather than `secrecy::SecretString`. The
/// [`Debug`] impl below is written by hand instead of derived so those two
/// fields are redacted in logs while remaining fully accessible to callers.
///
/// [ADR 0005]: https://github.com/constructorfabric/gears-rust/blob/main/docs/arch/authorization/ADR/0005-verified-principal-request-extension.md
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedPrincipal {
    /// Internal subject identifier (matches `SecurityContext::subject_id`).
    pub subject_id: Uuid,

    /// External subject as issued by the `IdP` (the `sub` claim / provider UID).
    pub external_subject: String,

    /// Token issuer (`iss`).
    pub issuer: String,

    /// Verified email, when present.
    pub email: Option<String>,

    /// Whether the `IdP` reports the email as verified, when the claim is present.
    pub email_verified: Option<bool>,

    /// Identity provider / sign-in method (e.g. `password`, `google.com`).
    pub provider: String,

    /// Whether the subject is anonymous (no durable identity).
    pub is_anonymous: bool,

    /// Token issued-at (`iat`), unix seconds.
    pub issued_at: i64,

    /// Time of the original authentication (`auth_time`), unix seconds.
    pub auth_time: i64,

    /// Token expiry (`exp`), unix seconds.
    pub expires_at: i64,
}

impl std::fmt::Debug for VerifiedPrincipal {
    /// Redacts `email` and `external_subject` (PII) so `{:?}`/`tracing`
    /// logging of this type or `AuthenticationResult` cannot leak them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedPrincipal")
            .field("subject_id", &self.subject_id)
            .field("external_subject", &"[redacted]")
            .field("issuer", &self.issuer)
            .field("email", &self.email.as_ref().map(|_| "[redacted]"))
            .field("email_verified", &self.email_verified)
            .field("provider", &self.provider)
            .field("is_anonymous", &self.is_anonymous)
            .field("issued_at", &self.issued_at)
            .field("auth_time", &self.auth_time)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Request to exchange `OAuth2` client credentials for a `SecurityContext`.
///
/// The caller provides its credentials; the `AuthN` plugin knows the token
/// endpoint / issuer URL from its own configuration.
pub struct ClientCredentialsRequest {
    /// `OAuth2` client identifier.
    pub client_id: String,

    /// `OAuth2` client secret.
    pub client_secret: SecretString,

    /// Optional scopes to request from the `IdP`.
    /// Passed as `scope` parameter in the `OAuth2` `client_credentials` grant.
    /// When empty, the `IdP` returns its default scopes.
    pub scopes: Vec<String>,
}
