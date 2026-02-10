pub mod domain_leader;
pub mod orchestrator;
pub mod specialist;

use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCall {
    pub agent_uid: String,
    pub tier: u8,
    pub persona: String,
    pub system_prompt: String,
    pub allowed_tools: Vec<String>,
    pub token_budget: u32,
    pub context: AgentContext,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContext {
    pub task_objective: String,
    pub parent_summary: Option<String>,
    pub code_snippets: Vec<CodeBlock>,
    pub constraints: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeBlock {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
}
