use serde::{Deserialize, Serialize};

use crate::mcp_bridge::client::BridgeClient;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeMcpConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRequest {
    pub action: String,
    pub target_project: String,
    pub path: Option<String>,
    pub pattern: Option<String>,
    pub limit: Option<u32>,
    pub mcp: Option<BridgeMcpConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTargetDirInput {
    pub target_project: String,
    pub dir_path: Option<String>,
    pub mcp_command: Option<String>,
    pub mcp_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTargetFileInput {
    pub target_project: String,
    pub file_path: String,
    pub mcp_command: Option<String>,
    pub mcp_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchTargetFilesInput {
    pub target_project: String,
    pub pattern: String,
    pub limit: Option<u32>,
    pub mcp_command: Option<String>,
    pub mcp_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub root: String,
    pub cwd: String,
    pub parent: Option<String>,
    pub entries: Vec<DirectoryEntry>,
    pub source: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetFileContent {
    pub root: String,
    pub path: String,
    pub size: u64,
    pub content: String,
    pub source: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub path: String,
    pub line: Option<u64>,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub root: String,
    pub pattern: String,
    pub matches: Vec<SearchMatch>,
    pub source: String,
    pub warnings: Vec<String>,
}

fn optional_mcp(command: Option<String>, args: Option<Vec<String>>) -> Option<BridgeMcpConfig> {
    let Some(command) = command.map(|value| value.trim().to_string()) else {
        return None;
    };

    if command.is_empty() {
        return None;
    }

    Some(BridgeMcpConfig {
        command,
        args: args.unwrap_or_default(),
    })
}

pub async fn list_dir(
    client: &BridgeClient,
    input: ListTargetDirInput,
) -> Result<DirectoryListing, String> {
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }

    let request = BridgeRequest {
        action: "list_dir".to_string(),
        target_project: input.target_project,
        path: input.dir_path,
        pattern: None,
        limit: None,
        mcp: optional_mcp(input.mcp_command, input.mcp_args),
    };

    client.call(&request).await
}

pub async fn read_file(
    client: &BridgeClient,
    input: ReadTargetFileInput,
) -> Result<TargetFileContent, String> {
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }
    if input.file_path.trim().is_empty() {
        return Err("filePath is required".to_string());
    }

    let request = BridgeRequest {
        action: "read_file".to_string(),
        target_project: input.target_project,
        path: Some(input.file_path),
        pattern: None,
        limit: None,
        mcp: optional_mcp(input.mcp_command, input.mcp_args),
    };

    client.call(&request).await
}

pub async fn search_files(
    client: &BridgeClient,
    input: SearchTargetFilesInput,
) -> Result<SearchResult, String> {
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }
    if input.pattern.trim().is_empty() {
        return Err("pattern is required".to_string());
    }

    let request = BridgeRequest {
        action: "search_files".to_string(),
        target_project: input.target_project,
        path: None,
        pattern: Some(input.pattern),
        limit: input.limit,
        mcp: optional_mcp(input.mcp_command, input.mcp_args),
    };

    client.call(&request).await
}
