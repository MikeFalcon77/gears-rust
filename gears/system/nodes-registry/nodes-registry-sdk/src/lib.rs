//! Nodes Registry SDK — public contract surface.
//!
//! This crate publishes the inter-gear client trait
//! ([`NodesRegistryClient`]) and its node data types. Per
//! [ADR 0005][adr] the trait boundary is
//! [`toolkit_canonical_errors::CanonicalError`]; the SDK additionally
//! ships [`NodesRegistryError`] as an **opt-in** typed projection
//! (`From<CanonicalError>`) for consumers that want flat dispatch on
//! the categories nodes-registry emits. The single authoritative
//! AIP-193 ladder (`From<DomainError> for CanonicalError`) lives in the
//! impl crate (`cf-gears-nodes-registry`); this SDK never re-derives
//! that classification — it only projects the finished `CanonicalError`.
//!
//! # Error surface
//!
//! Trait methods return `Result<_, CanonicalError>`. Consumers may
//! propagate the canonical error, or project it into the typed
//! [`NodesRegistryError`] view via
//! `.map_err(NodesRegistryError::from)` (or a
//! `From<CanonicalError> for OwnError` chain). The projection models the
//! three canonical categories nodes-registry emits (`NotFound`,
//! `InvalidArgument`, `Internal`), plus a catch-all `Other { canonical }`;
//! see [`error`] for the full dispatch table, the three consumer
//! integration patterns, and the co-located wire vocabulary ([`field`],
//! [`gts`]).
//!
//! [adr]: https://github.com/constructorfabric/gears-rust/blob/main/docs/arch/errors/ADR/0005-cpt-cf-adr-sdk-canonical-projection.md
#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod api;
pub mod error;
pub mod field;
pub mod gts;

pub use api::NodesRegistryClient;
pub use error::NodesRegistryError;
pub use gts::NODE_RESOURCE_TYPE;

pub use toolkit_node_info::{
    BatteryInfo, CpuInfo, GpuInfo, HostInfo, MemoryInfo, Node, NodeSysCap, NodeSysInfo, OsInfo,
    SysCap,
};
