use toolkit_macros::domain_model;

/// Domain-level errors for nodes registry
#[domain_model]
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Node not found: {0}")]
    NodeNotFound(uuid::Uuid),

    #[error("Failed to collect system information: {0}")]
    SysInfoCollectionFailed(String),

    #[error("Failed to collect system capabilities: {0}")]
    SysCapCollectionFailed(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[allow(unknown_lints, de1302_error_from_to_string)]
impl From<anyhow::Error> for DomainError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<toolkit_node_info::NodeInfoError> for DomainError {
    fn from(e: toolkit_node_info::NodeInfoError) -> Self {
        match e {
            toolkit_node_info::NodeInfoError::SysInfoCollectionFailed(msg) => {
                Self::SysInfoCollectionFailed(msg)
            }
            toolkit_node_info::NodeInfoError::SysCapCollectionFailed(msg) => {
                Self::SysCapCollectionFailed(msg)
            }
            toolkit_node_info::NodeInfoError::HardwareUuidFailed(msg) => {
                Self::Internal(format!("Hardware UUID failed: {msg}"))
            }
            toolkit_node_info::NodeInfoError::Internal(msg) => Self::Internal(msg),
        }
    }
}
