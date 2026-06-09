//! Wire `field` / `reason` vocabulary for field violations under
//! [`CanonicalError::InvalidArgument`].
//!
//! The reason constant is a stable machine-readable code that lands in
//! `CanonicalError::InvalidArgument.ctx.field_violations[].reason` when
//! the nodes registry rejects request input; the field constant is the
//! corresponding `field_violations[].field` attribution.
//!
//! nodes-registry emits exactly one reason (`VALIDATION_ERROR`), so the
//! projected [`crate::NodesRegistryError::InvalidArgument`] carries the
//! `reason` as a plain `String` rather than a typed sub-enum — there is
//! no second disposition to `match` on. A typed `ValidationReason` view
//! can be introduced later if the emission set grows; this gear is the
//! seam for that. The impl crate's single
//! `From<DomainError> for CanonicalError` ladder references the same
//! constants at construction time so the SDK vocabulary and the wire can
//! never drift — the round-trip tests in [`crate::error`] pin every
//! constant to its `Problem` JSON path.
//!
//! [`CanonicalError::InvalidArgument`]: toolkit_canonical_errors::CanonicalError::InvalidArgument

/// Generic request-input validation failure (e.g. malformed capability
/// key). The only `field_violations[].reason` nodes-registry emits.
pub const VALIDATION_ERROR: &str = "VALIDATION_ERROR";

/// `field_violations[].field` attribution for [`VALIDATION_ERROR`] — the
/// rejected request input. Not a dispatch discriminator; extracted to a
/// const (ADR 0005 Rule 6) so the impl ladder and the SDK vocabulary
/// cannot drift, pinned by the round-trip tests.
pub const INPUT_FIELD: &str = "input";
