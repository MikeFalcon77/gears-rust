use toolkit_canonical_errors::CanonicalError;

use crate::{Node, NodeSysCap, NodeSysInfo};

/// Client trait for accessing nodes registry functionality.
///
/// Per [ADR 0005][adr] every fallible method returns
/// `Result<_, CanonicalError>` — the canonical error envelope is the
/// contract at this in-process boundary. Consumers that prefer flat
/// dispatch over the categories nodes-registry emits may project the
/// envelope into the opt-in [`crate::NodesRegistryError`] view via
/// `.map_err(NodesRegistryError::from)`; see its gear docs for the
/// dispatch table and consumer patterns.
///
/// [adr]: https://github.com/constructorfabric/gears-rust/blob/main/docs/arch/errors/ADR/0005-cpt-cf-adr-sdk-canonical-projection.md
#[async_trait::async_trait]
pub trait NodesRegistryClient: Send + Sync {
    /// Get a node by ID
    async fn get_node(&self, id: uuid::Uuid) -> Result<Node, CanonicalError>;

    /// List all nodes
    async fn list_nodes(&self) -> Result<Vec<Node>, CanonicalError>;

    /// Get system information for a node
    async fn get_node_sysinfo(&self, node_id: uuid::Uuid) -> Result<NodeSysInfo, CanonicalError>;

    /// Get system capabilities for a node
    async fn get_node_syscap(&self, node_id: uuid::Uuid) -> Result<NodeSysCap, CanonicalError>;
}
