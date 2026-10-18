//! Configurable Firebase `AuthN` plugin, derived from the owner's boring-auth adapter.
//! The verifier preserves RS256/JWK validation; identity and tenant selection are deployment data.
pub mod config;
pub mod firebase;
mod gear;
pub use gear::FirebaseAuthnPlugin;
