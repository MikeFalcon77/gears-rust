// Adapted from boring-pills/crates/boring-auth/src/firebase.rs at the owner's request.
// Generalized for configurable Firebase identities; no project or account is built in.
//! Verifying a Firebase ID token, and deciding whether it belongs to an admin.
//!
//! # The thing to get right
//!
//! Firebase social login will issue a perfectly valid ID token to **anybody**
//! with a Google account. Signing in is not a decision this project makes; it
//! is a button. So a token that verifies proves who somebody is and says
//! nothing whatever about whether they may be here.
//!
//! The uid+email check against configured lists is therefore not a detail of
//! this module, it is the whole of the access control. Omitting it — or
//! "temporarily" relaxing it — hands the admin surface to the internet, and
//! everything else in this file would still pass.
//!
//! # Which keys
//!
//! Firebase ID tokens are signed by `securetoken@system.gserviceaccount.com`,
//! and the keys live at the JWK endpoint named below. The nearby
//! `oauth2/v3/certs` is a different key set for Google's *OAuth* ID tokens, and
//! using it here would refuse every genuine token with a signature error —
//! which reads as "Firebase is broken" rather than "wrong endpoint".

use std::collections::HashMap;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::RwLock;
use toolkit_http::HttpClient;
use tracing::{debug, warn};

/// Google's public keys for Firebase ID tokens, in JWK form.
const JWK_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

/// Used only when the response carries no usable `Cache-Control: max-age`.
/// Google rotates daily and always sends one; this is the floor under a
/// malformed header, not the normal path.
const FALLBACK_KEY_TTL: Duration = Duration::from_hours(1);

/// The claims worth reading. Everything else Firebase sends is either checked
/// by the library or of no interest here.
#[derive(Debug, Deserialize)]
struct Claims {
    /// The Firebase user id. Stable across email changes and sign-in methods,
    /// which is why the allowlist is keyed on it rather than on an address.
    sub: String,
    iat: u64,
    #[serde(default)]
    auth_time: Option<u64>,
    #[serde(default)]
    email: Option<String>,
    /// Google can put an address on the token before the owner has confirmed
    /// it. An unverified claim is treated as absent.
    #[serde(default)]
    email_verified: bool,
}

struct CachedKeys {
    keys: JwkSet,
    fetched_at: Instant,
    ttl: Duration,
}

impl CachedKeys {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < self.ttl
    }
}

/// Why a token was not accepted.
///
/// Separate variants for the log. What the caller turns them into is one
/// `Unauthorized` with one message, because a person who is not an admin
/// learning *why* only tells them how close they got.
#[derive(Debug)]
pub enum Refusal {
    /// Malformed, wrong algorithm, wrong issuer, expired, or badly signed.
    NotAValidToken(String),
    /// Genuine, and for somebody who is not an admin here.
    IdentityNotAllowed { uid: String, email: Option<String> },
    /// Google's keys could not be fetched. Not the caller's fault, and not
    /// something to answer 401 to.
    KeysUnavailable(String),
}

pub struct FirebaseVerifier {
    project_id: String,
    identities: Vec<(String, String)>,
    /// `None` only in tests, which verify against a key of their own and never
    /// reach Google. A missing client at runtime would be a construction bug,
    /// and it refuses rather than panicking.
    http: Option<HttpClient>,
    keys: RwLock<Option<CachedKeys>>,
}

#[derive(Debug, thiserror::Error)]
pub enum FirebaseInitError {
    #[error("building the Firebase key client")]
    Http(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl FirebaseVerifier {
    /// Must be built inside a Tokio runtime: the HTTP client spawns a tower
    /// worker. `Gear::init` is async, which is where this happens.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    pub fn new(
        project_id: String,
        identities: Vec<(String, String)>,
    ) -> Result<Self, FirebaseInitError> {
        let http = HttpClient::builder()
            .with_otel()
            .timeout(Duration::from_secs(5))
            .total_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| FirebaseInitError::Http(Box::new(e)))?;
        Ok(Self {
            project_id,
            identities,
            http: Some(http),
            keys: RwLock::new(None),
        })
    }

    /// A verifier that cannot fetch keys. Used to pin the 503 mapping for a
    /// key-fetch outage, and by unit tests that inject a signing key of their
    /// own.
    #[must_use]
    pub fn without_http(project_id: String, identities: Vec<(String, String)>) -> Self {
        Self {
            project_id,
            identities,
            http: None,
            keys: RwLock::new(None),
        }
    }

    /// The uid behind an admin's ID token.
    ///
    /// # Errors
    ///
    /// [`Refusal`] for every reason the token is not an admin's, including the
    /// one that is not the caller's fault.
    pub async fn verify_identity(&self, token: &str) -> Result<String, Refusal> {
        if token.len() > 8192 {
            return Err(Refusal::NotAValidToken(
                "token exceeds size limit".to_owned(),
            ));
        }
        let header =
            decode_header(token).map_err(|e| Refusal::NotAValidToken(format!("header: {e}")))?;

        // Pinned rather than taken from the token. `alg` is attacker-supplied,
        // and honouring it is how a token signed with `none`, or with the
        // public key as an HMAC secret, gets accepted.
        if header.alg != Algorithm::RS256 {
            return Err(Refusal::NotAValidToken(format!(
                "algorithm {:?}, expected RS256",
                header.alg
            )));
        }
        let kid = header
            .kid
            .ok_or_else(|| Refusal::NotAValidToken("no key id".to_owned()))?;

        if kid.is_empty() || kid.len() > 256 {
            return Err(Refusal::NotAValidToken("invalid key id".to_owned()));
        }
        let key = self.key_for(&kid).await?;
        self.check(token, &key)
    }

    /// Everything after the signing key has been found.
    ///
    /// Separated so it can be tested against a key of our own. Fetching
    /// Google's would make every one of those tests a network call, and the
    /// rules that matter here -- audience, issuer, expiry, and above all the
    /// allowlist -- have nothing to do with where the key came from.
    fn check(&self, token: &str, key: &DecodingKey) -> Result<String, Refusal> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.leeway = 0;
        validation.validate_nbf = true;
        validation.set_audience(&[&self.project_id]);
        validation.set_issuer(&[format!(
            "https://securetoken.google.com/{}",
            self.project_id
        )]);
        // `sub` is the allowlist key, so a token without one must not reach the
        // comparison with an empty string.
        validation.required_spec_claims = ["exp", "iat", "aud", "iss", "sub"]
            .iter()
            .map(|c| (*c).to_owned())
            .collect();

        let data = decode::<Claims>(token, key, &validation)
            .map_err(|e| Refusal::NotAValidToken(e.to_string()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Refusal::KeysUnavailable(e.to_string()))?
            .as_secs();
        if data.claims.iat > now.saturating_add(60)
            || data
                .claims
                .auth_time
                .is_some_and(|time| time > now.saturating_add(60))
        {
            return Err(Refusal::NotAValidToken(
                "token was issued in the future".to_owned(),
            ));
        }
        let uid = data.claims.sub;
        if uid.is_empty() {
            return Err(Refusal::NotAValidToken("empty subject".to_owned()));
        }

        let email = data.claims.email.filter(|value| !value.trim().is_empty());

        let matched = email.as_deref().is_some_and(|got| {
            data.claims.email_verified
                && self.identities.iter().any(|(allowed_uid, allowed_email)| {
                    allowed_uid == &uid && allowed_email.eq_ignore_ascii_case(got)
                })
        });
        if !matched {
            return Err(Refusal::IdentityNotAllowed { uid, email });
        }

        Ok(uid)
    }

    /// One signing key, from cache when the cache is still good.
    async fn key_for(&self, kid: &str) -> Result<DecodingKey, Refusal> {
        if let Some(key) = self.cached_key(kid).await? {
            return Ok(key);
        }

        self.refresh_keys().await?;

        // A `kid` that is still missing after a fresh fetch is a property of the
        // token, not of the cache -- so this is a refusal rather than a retry.
        self.cached_key(kid).await?.ok_or_else(|| {
            Refusal::NotAValidToken(format!("no published key matches key id {kid}"))
        })
    }

    async fn cached_key(&self, kid: &str) -> Result<Option<DecodingKey>, Refusal> {
        let guard = self.keys.read().await;
        let Some(cached) = guard.as_ref() else {
            return Ok(None);
        };
        if !cached.is_fresh() {
            return Ok(None);
        }
        let Some(jwk) = cached.keys.find(kid) else {
            return Ok(None);
        };
        DecodingKey::from_jwk(jwk).map(Some).map_err(|e| {
            warn!(error = %e, %kid, "firebase jwk could not be decoded");
            Refusal::KeysUnavailable(format!("jwk {kid}: {e}"))
        })
    }

    async fn refresh_keys(&self) -> Result<(), Refusal> {
        let http = self
            .http
            .as_ref()
            .ok_or_else(|| Refusal::KeysUnavailable("no http client".to_owned()))?;

        let response = http
            .get(JWK_URL)
            .send()
            .await
            .map_err(|e| Refusal::KeysUnavailable(e.to_string()))?;

        let ttl = max_age(response.headers()).unwrap_or(FALLBACK_KEY_TTL);

        let body = response
            .bytes()
            .await
            .map_err(|e| Refusal::KeysUnavailable(e.to_string()))?;

        let keys: JwkSet = serde_json::from_slice(&body)
            .map_err(|e| Refusal::KeysUnavailable(format!("key set: {e}")))?;

        debug!(
            count = keys.keys.len(),
            ttl_secs = ttl.as_secs(),
            "fetched Google signing keys"
        );

        *self.keys.write().await = Some(CachedKeys {
            keys,
            fetched_at: Instant::now(),
            ttl,
        });
        Ok(())
    }
}

/// How long Google says its keys are good for.
///
/// Honoured rather than assumed: the keys rotate daily, and caching past the
/// window means refusing genuine tokens for however long the cache outlives the
/// key -- a failure that looks like everybody being logged out at once.
fn max_age(headers: &http::HeaderMap) -> Option<Duration> {
    let value = headers.get(http::header::CACHE_CONTROL)?.to_str().ok()?;
    let directives: HashMap<&str, &str> = value
        .split(',')
        .filter_map(|part| part.trim().split_once('='))
        .collect();
    let seconds: u64 = directives.get("max-age")?.trim().parse().ok()?;
    if seconds == 0 {
        warn!("google returned max-age=0 for its signing keys");
        return None;
    }
    Some(Duration::from_secs(seconds))
}

#[cfg(test)]
pub(crate) fn with_test_keys(
    project_id: String,
    identities: Vec<(String, String)>,
    keys: JwkSet,
) -> FirebaseVerifier {
    FirebaseVerifier {
        project_id,
        identities,
        http: None,
        keys: RwLock::new(Some(CachedKeys {
            keys,
            fetched_at: Instant::now(),
            ttl: Duration::from_secs(3600),
        })),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, reason = "RSA test fixtures")]
pub(crate) mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use rsa::{
        RsaPrivateKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
    };
    use serde::Serialize;
    use std::sync::OnceLock;

    const PROJECT: &str = "firebase-authn-test";
    const ADMIN_UID: &str = "firebase-uid-of-the-owner";
    const ADMIN_EMAIL: &str = "owner@example.com";

    /// A keypair of our own, so these tests exercise the rules rather than the
    /// network. Google's key is not what any of them is about.
    fn generate_keys() -> (String, String) {
        let private = RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).expect("test RSA key");
        let public = private
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("test public PEM");
        let private = private
            .to_pkcs8_pem(LineEnding::LF)
            .expect("test private PEM");
        (private.to_string(), public)
    }
    pub fn test_keys() -> &'static (String, String) {
        static KEYS: OnceLock<(String, String)> = OnceLock::new();
        KEYS.get_or_init(generate_keys)
    }

    #[derive(Serialize)]
    struct TestClaims {
        sub: String,
        aud: String,
        iss: String,
        exp: i64,
        iat: i64,
        email: String,
        email_verified: bool,
    }

    fn verifier() -> FirebaseVerifier {
        // Not `new`: that builds an HTTP client, which needs a runtime and is
        // no part of what is under test here.
        FirebaseVerifier {
            project_id: PROJECT.to_owned(),
            identities: vec![(ADMIN_UID.to_owned(), ADMIN_EMAIL.to_owned())],
            http: None,
            keys: RwLock::new(None),
        }
    }

    fn key() -> DecodingKey {
        DecodingKey::from_rsa_pem(test_keys().1.as_bytes()).expect("test public key")
    }

    fn token_for(uid: &str) -> String {
        token_with(uid, ADMIN_EMAIL, true)
    }

    fn token_with(uid: &str, email: &str, email_verified: bool) -> String {
        signed(
            uid,
            email,
            email_verified,
            PROJECT,
            &format!("https://securetoken.google.com/{PROJECT}"),
            3600,
        )
    }

    fn signed(
        uid: &str,
        email: &str,
        email_verified: bool,
        aud: &str,
        iss: &str,
        expires_in: i64,
    ) -> String {
        let now = chrono::Utc::now().timestamp();
        let claims = TestClaims {
            sub: uid.to_owned(),
            aud: aud.to_owned(),
            iss: iss.to_owned(),
            exp: now + expires_in,
            iat: now - 10,
            email: email.to_owned(),
            email_verified,
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key".to_owned());
        encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(test_keys().0.as_bytes()).expect("test private key"),
        )
        .expect("signed")
    }

    #[test]
    fn the_configured_admin_is_admitted() {
        let uid = verifier()
            .check(&token_for(ADMIN_UID), &key())
            .expect("the admin must get in");
        assert_eq!(uid, ADMIN_UID);
    }

    /// The test this whole module exists for.
    ///
    /// Firebase will issue a genuine, correctly signed, unexpired token for
    /// this project to anybody who clicks "Sign in with Google". Every other
    /// check in `check` passes for that token. The allowlist is the only thing
    /// between them and the admin surface, and dropping it is a one-line edit
    /// that no other test here would notice.
    #[test]
    fn a_google_account_outside_the_admin_list_is_refused() {
        let refusal = verifier()
            .check(&token_for("some-other-google-user"), &key())
            .expect_err("a stranger with a valid token must be refused");

        assert!(
            matches!(&refusal, Refusal::IdentityNotAllowed { uid, .. } if uid == "some-other-google-user"),
            "{refusal:?}"
        );
    }

    /// Right uid, somebody else's address on the same genuine token.
    #[test]
    fn the_right_uid_with_the_wrong_email_is_refused() {
        let refusal = verifier()
            .check(&token_with(ADMIN_UID, "stranger@example.com", true), &key())
            .expect_err("uid alone must not admit a different address");

        assert!(
            matches!(&refusal, Refusal::IdentityNotAllowed { uid, .. } if uid == ADMIN_UID),
            "{refusal:?}"
        );
    }

    /// Right address, somebody else's Firebase uid.
    #[test]
    fn the_right_email_with_the_wrong_uid_is_refused() {
        let refusal = verifier()
            .check(&token_for("some-other-google-user"), &key())
            .expect_err("email alone must not admit a different uid");

        assert!(
            matches!(&refusal, Refusal::IdentityNotAllowed { uid, .. } if uid == "some-other-google-user"),
            "{refusal:?}"
        );
    }

    #[test]
    fn an_unverified_admin_email_is_refused() {
        let refusal = verifier()
            .check(&token_with(ADMIN_UID, ADMIN_EMAIL, false), &key())
            .expect_err("an unverified claim must not count as the allowlisted address");

        assert!(
            matches!(&refusal, Refusal::IdentityNotAllowed { uid, .. } if uid == ADMIN_UID),
            "{refusal:?}"
        );
    }

    /// Signed by the same Google key, minted for a different project.
    #[test]
    fn a_token_for_another_firebase_project_is_refused() {
        let other = signed(
            ADMIN_UID,
            ADMIN_EMAIL,
            true,
            "somebody-elses-project",
            "https://securetoken.google.com/somebody-elses-project",
            3600,
        );
        assert!(matches!(
            verifier().check(&other, &key()),
            Err(Refusal::NotAValidToken(_))
        ));
    }

    /// The right project, the right admin, and the wrong issuer: a token minted
    /// by something that is not Firebase.
    #[test]
    fn a_token_from_the_wrong_issuer_is_refused() {
        let forged = signed(
            ADMIN_UID,
            ADMIN_EMAIL,
            true,
            PROJECT,
            "https://example.com/",
            3600,
        );
        assert!(matches!(
            verifier().check(&forged, &key()),
            Err(Refusal::NotAValidToken(_))
        ));
    }

    #[test]
    fn an_expired_id_token_is_refused() {
        let stale = signed(
            ADMIN_UID,
            ADMIN_EMAIL,
            true,
            PROJECT,
            &format!("https://securetoken.google.com/{PROJECT}"),
            // Past the default 60s leeway, so this is expiry rather than skew.
            -600,
        );
        assert!(matches!(
            verifier().check(&stale, &key()),
            Err(Refusal::NotAValidToken(_))
        ));
    }

    /// A token signed by a key that is not the one we verify with.
    #[test]
    fn a_token_signed_by_an_unknown_key_is_refused() {
        let wrong_key = DecodingKey::from_rsa_pem(generate_keys().1.as_bytes()).expect("other key");
        assert!(matches!(
            verifier().check(&token_for(ADMIN_UID), &wrong_key),
            Err(Refusal::NotAValidToken(_))
        ));
    }

    #[test]
    fn the_key_url_is_the_firebase_one_not_the_oauth_one() {
        // These differ by one path and fail in opposite directions: the OAuth
        // set verifies nothing Firebase signs, so every genuine admin token
        // would be refused with a signature error.
        assert!(JWK_URL.contains("securetoken@system.gserviceaccount.com"));
        assert!(!JWK_URL.contains("oauth2"));
    }

    #[test]
    fn a_cache_lifetime_is_read_from_the_response() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::CACHE_CONTROL,
            "public, max-age=19008, must-revalidate, no-transform"
                .parse()
                .unwrap(),
        );
        assert_eq!(max_age(&headers), Some(Duration::from_secs(19_008)));
    }

    #[test]
    fn a_missing_or_useless_cache_header_falls_back_rather_than_caching_forever() {
        assert_eq!(max_age(&http::HeaderMap::new()), None);

        let mut zero = http::HeaderMap::new();
        zero.insert(http::header::CACHE_CONTROL, "max-age=0".parse().unwrap());
        assert_eq!(max_age(&zero), None);
    }
    pub fn signing_keys() -> JwkSet {
        use base64::Engine as _;
        use rsa::pkcs8::DecodePublicKey;
        use rsa::traits::PublicKeyParts;
        let public = rsa::RsaPublicKey::from_public_key_pem(&test_keys().1).unwrap();
        serde_json::from_value(serde_json::json!({"keys": [{"kty": "RSA", "kid": "test-key", "alg": "RS256", "use": "sig",
            "n": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),
            "e": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public.e().to_bytes_be())}]})).unwrap()
    }

    pub fn signed_claims(extra: serde_json::Value) -> String {
        let now = chrono::Utc::now().timestamp();
        let mut claims = serde_json::json!({"sub": ADMIN_UID, "aud": PROJECT, "iss": format!("https://securetoken.google.com/{PROJECT}"),
            "iat": now - 10, "exp": now + 3600, "email": ADMIN_EMAIL, "email_verified": true});
        let serde_json::Value::Object(extra) = extra else {
            panic!("test claim overrides must be an object")
        };
        claims.as_object_mut().unwrap().extend(extra);
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key".to_owned());
        encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(test_keys().0.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn cached_jwk_verifies_the_complete_token_path() {
        let verifier = with_test_keys(
            PROJECT.to_owned(),
            vec![(ADMIN_UID.to_owned(), ADMIN_EMAIL.to_owned())],
            signing_keys(),
        );
        assert_eq!(
            verifier
                .verify_identity(&token_for(ADMIN_UID))
                .await
                .unwrap(),
            ADMIN_UID
        );
    }

    #[test]
    fn future_issue_and_authentication_times_are_rejected() {
        let future = chrono::Utc::now().timestamp() + 3600;
        for field in ["iat", "auth_time", "nbf"] {
            let token = signed_claims(serde_json::json!({field: future}));
            assert!(
                matches!(
                    verifier().check(&token, &key()),
                    Err(Refusal::NotAValidToken(_))
                ),
                "future {field}"
            );
        }
    }

    #[test]
    fn missing_issue_time_and_just_expired_tokens_are_rejected() {
        let mut claims = serde_json::json!({"sub": ADMIN_UID, "aud": PROJECT, "iss": format!("https://securetoken.google.com/{PROJECT}"),
            "exp": chrono::Utc::now().timestamp() + 3600, "email": ADMIN_EMAIL, "email_verified": true});
        let header = Header::new(Algorithm::RS256);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(test_keys().0.as_bytes()).unwrap(),
        )
        .unwrap();
        assert!(verifier().check(&token, &key()).is_err());
        claims["iat"] = serde_json::json!(chrono::Utc::now().timestamp() - 100);
        claims["exp"] = serde_json::json!(chrono::Utc::now().timestamp() - 1);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(test_keys().0.as_bytes()).unwrap(),
        )
        .unwrap();
        assert!(verifier().check(&token, &key()).is_err());
    }

    #[tokio::test]
    async fn missing_or_unknown_kid_and_key_outage_fail_closed() {
        let verifier = FirebaseVerifier::without_http(
            PROJECT.to_owned(),
            vec![(ADMIN_UID.to_owned(), ADMIN_EMAIL.to_owned())],
        );
        assert!(matches!(
            verifier.verify_identity(&token_for(ADMIN_UID)).await,
            Err(Refusal::KeysUnavailable(_))
        ));
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("unknown-key".to_owned());
        let token = encode(
            &header,
            &serde_json::json!({}),
            &EncodingKey::from_rsa_pem(test_keys().0.as_bytes()).unwrap(),
        )
        .unwrap();
        let cached = with_test_keys(
            PROJECT.to_owned(),
            vec![(ADMIN_UID.to_owned(), ADMIN_EMAIL.to_owned())],
            signing_keys(),
        );
        assert!(cached.verify_identity(&token).await.is_err());
        header.kid = None;
        let token = encode(
            &header,
            &serde_json::json!({}),
            &EncodingKey::from_rsa_pem(test_keys().0.as_bytes()).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            cached.verify_identity(&token).await,
            Err(Refusal::NotAValidToken(_))
        ));
    }

    #[tokio::test]
    async fn symmetric_algorithm_and_oversized_tokens_are_rejected_before_key_fetch() {
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some("test-key".to_owned());
        let token = encode(
            &header,
            &serde_json::json!({}),
            &EncodingKey::from_secret(b"not-a-firebase-key"),
        )
        .unwrap();
        assert!(matches!(
            verifier().verify_identity(&token).await,
            Err(Refusal::NotAValidToken(_))
        ));
        assert!(matches!(
            verifier().verify_identity(&"a".repeat(8193)).await,
            Err(Refusal::NotAValidToken(_))
        ));
    }
}
