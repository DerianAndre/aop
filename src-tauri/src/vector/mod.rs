pub mod indexer;
pub mod search;

use serde::{Deserialize, Serialize};

pub const VECTOR_DIM: usize = 256;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProjectInput {
    pub target_project: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProjectResult {
    pub target_project: String,
    pub table_name: String,
    pub indexed_files: u32,
    pub indexed_chunks: u32,
    pub index_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryCodebaseInput {
    pub target_project: String,
    pub query: String,
    pub top_k: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextChunk {
    pub id: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub chunk_type: String,
    pub name: String,
    pub content: String,
    pub score: f32,
}
