use serde::{Deserialize, Serialize};

/// Fork: `fs.list_dirs` — subfolders of `path`, or of the server's home when omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
pub struct FsListDirsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}
