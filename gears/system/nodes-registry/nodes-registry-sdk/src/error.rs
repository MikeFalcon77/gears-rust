//! Nodes Registry SDK error surface — typed projection of
//! [`CanonicalError`].
//!
//! # Opt-in convenience, not the contract
//!
//! Per [ADR 0005][adr] the [`NodesRegistryClient`] trait boundary is
//! `Result<_, CanonicalError>`. [`NodesRegistryError`] is an **opt-in**
//! typed view over that envelope, shipped for consumers that want flat
//! dispatch on the categories nodes-registry emits. It is *not* part of
//! the trait contract: adding a variant is non-breaking, and the single
//! authoritative AIP-193 classification lives in the impl crate's one
//! `From<DomainError> for CanonicalError` ladder — this projection only
//! reads the finished `CanonicalError`.
//!
//! The conversion is infallible (`From<CanonicalError>`). Canonical
//! categories nodes-registry does not emit fall through to
//! [`NodesRegistryError::Other`], which preserves the full
//! [`CanonicalError`] for inspection / forward-compatible dispatch.
//!
//! # What nodes-registry emits — consumer dispatch reference
//!
//! | Disposition | Match arm | HTTP |
//! |---|---|---|
//! | node id does not resolve | [`NodesRegistryError::NotFound`] | 404 |
//! | request-input validation (e.g. bad capability key) | [`NodesRegistryError::InvalidArgument`] | 400 |
//! | sysinfo/syscap collection failure, internal error | [`NodesRegistryError::Internal`] | 500 |
//! | anything else (forward-compat) | [`NodesRegistryError::Other`] | — |
//!
//! [`NodesRegistryError::NotFound`] carries the raw `resource_type`;
//! match it against [`crate::gts::NODE_RESOURCE_TYPE`].
//! [`NodesRegistryError::InvalidArgument`] carries the wire `reason`
//! (currently only [`crate::field::VALIDATION_ERROR`]) and `field`.
//!
//! # Consumer integration — three patterns
//!
//! **Pattern 1 — pure propagation (no projection):**
//!
//! ```ignore
//! let node = nodes_client.get_node(id).await?; // ? propagates CanonicalError
//! ```
//!
//! **Pattern 2 — explicit projection at the call site:**
//!
//! ```ignore
//! use nodes_registry_sdk::NodesRegistryError;
//!
//! match nodes_client.get_node(id).await.map_err(NodesRegistryError::from) {
//!     Err(NodesRegistryError::NotFound { .. }) => /* unknown node */,
//!     Err(NodesRegistryError::InvalidArgument { field, .. }) => /* bad input */,
//!     _ => /* … */,
//! }
//! ```
//!
//! **Pattern 3 — transparent chaining via `From<CanonicalError> for OwnError`:**
//!
//! ```ignore
//! impl From<CanonicalError> for OwnConsumerError {
//!     fn from(err: CanonicalError) -> Self {
//!         NodesRegistryError::from(err).into() // route through the typed view
//!     }
//! }
//! // then every call site stays plain `?`.
//! ```
//!
//! Out-of-process consumers reconstruct the canonical error from the
//! wire via `TryFrom<Problem> for CanonicalError` first, then project:
//! `Problem JSON → Problem → CanonicalError → NodesRegistryError`.
//!
//! [`NodesRegistryClient`]: crate::NodesRegistryClient
//! [adr]: https://github.com/constructorfabric/gears-rust/blob/main/docs/arch/errors/ADR/0005-cpt-cf-adr-sdk-canonical-projection.md

use thiserror::Error;
use toolkit_canonical_errors::{CanonicalError, InvalidArgument};

/// Typed projection of [`CanonicalError`] for nodes-registry consumers.
///
/// The impl crate's `From<DomainError> for CanonicalError` is the single
/// authoritative AIP-193 mapping; this enum is a forward-compatible,
/// flat view over the three categories nodes-registry emits, plus the
/// mandatory catch-all [`Self::Other`]. See the [gear docs](self) for
/// the dispatch table and consumer patterns.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum NodesRegistryError {
    /// Node not found. `resource_type` is the canonical GTS type — match
    /// it against [`crate::gts::NODE_RESOURCE_TYPE`]; `name` is the raw
    /// node id the caller supplied.
    #[error("not found [{resource_type}]: {name}")]
    NotFound {
        resource_type: String,
        name: String,
        detail: String,
    },

    /// Request-input validation failure. `reason` is the wire code
    /// (currently only [`crate::field::VALIDATION_ERROR`]); `field` is
    /// the attributed request input.
    #[error("invalid argument [{field}/{reason}]: {detail}")]
    InvalidArgument {
        field: String,
        reason: String,
        detail: String,
    },

    /// Internal failure — sysinfo/syscap collection failure or an
    /// internal error (HTTP 500). Detail is intentionally opaque.
    #[error("internal error: {detail}")]
    Internal { detail: String },

    /// Forward-compatibility catch-all for any canonical category
    /// nodes-registry does not model above. Preserves the full
    /// [`CanonicalError`] for inspection.
    #[error("{canonical}")]
    Other { canonical: CanonicalError },
}

impl From<CanonicalError> for NodesRegistryError {
    fn from(err: CanonicalError) -> Self {
        let detail = err.detail().to_owned();
        match err {
            CanonicalError::NotFound {
                resource_type,
                resource_name,
                ..
            } => Self::NotFound {
                resource_type: resource_type.unwrap_or_default(),
                name: resource_name.unwrap_or_default(),
                detail,
            },

            CanonicalError::InvalidArgument { ctx, .. } => project_invalid_argument(ctx, detail),

            CanonicalError::Internal { .. } => Self::Internal { detail },

            other => Self::Other { canonical: other },
        }
    }
}

fn project_invalid_argument(ctx: InvalidArgument, detail: String) -> NodesRegistryError {
    // nodes-registry only ever emits the `FieldViolations` shape with a
    // single violation; the `Format` / `Constraint` shapes and the empty
    // case fall back to the canonical detail with an empty field/reason.
    let first_violation = match ctx {
        InvalidArgument::FieldViolations { field_violations } => {
            field_violations.into_iter().next()
        }
        InvalidArgument::Format { .. } | InvalidArgument::Constraint { .. } => None,
    };

    first_violation.map_or(
        NodesRegistryError::InvalidArgument {
            field: String::new(),
            reason: String::new(),
            detail,
        },
        |v| NodesRegistryError::InvalidArgument {
            field: v.field,
            reason: v.reason,
            detail: v.description,
        },
    )
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
