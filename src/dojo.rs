use compose_spec::Identifier;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DojoAssignmentError {
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("JSON error")]
    Serde(#[from] serde_json::Error),
}

/// Dojo assignment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DojoAssignment {
    /// I have absolutely NO idea what this is for
    pub dojo_assignment_version: u32,

    /// Neither do I know what this is for
    pub version: u32,

    /// A list of immutable files... for reasons?!
    pub immutable: Vec<DojoImmutableFileDescriptor>,

    /// The reference to the result (both the container and the volume)
    /// This might be the ONLY shred of valuable information in this file...
    pub result: DojoResult,
}

/// Immutable file descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DojoImmutableFileDescriptor {
    /// A description of the file, again, for reasons
    pub description: Option<String>,

    /// The path to the file
    pub path: String,

    /// Whether the file is a directory or not, because apparently that's important
    pub is_directory: Option<bool>,
}

/// Result configuration for a Dojo assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DojoResult {
    /// The container where the result is generated (?)
    pub container: Identifier,

    /// The volume where the result is stored
    pub volume: Option<Identifier>,
}

impl DojoAssignment {
    pub fn try_from_file(path: &Path) -> Result<Self, DojoAssignmentError> {
        let file = std::fs::read_to_string(path)?;
        serde_json::from_str(&file).map_err(Into::into)
    }
}
