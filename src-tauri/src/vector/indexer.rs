use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::vector::{IndexProjectResult, VECTOR_DIM};

const MAX_LINES_PER_CHUNK: usize = 180;

#[derive(Debug, Clone)]
struct ChunkRow {
    id: String,
    project_root: String,
    file_path: String,
    start_line: i64,
    end_line: i64,
    chunk_type: String,
    name: String,
    content: String,
    vector: Vec<f32>,
}

pub async fn index_project(
    pool: &SqlitePool,
    target_project: &str,
) -> Result<IndexProjectResult, String> {
    let target_root = normalize_project_root(target_project)?;
    let project_root_str = target_root.to_string_lossy().to_string();
    let table_name = table_name_for_project(&target_root);
    let files = collect_source_files(&target_root)?;

    let mut chunks: Vec<ChunkRow> = Vec::new();
    for file in &files {
        let content = match fs::read_to_string(file) {
            Ok(data) => data,
            Err(_) => continue,
        };
        let relative_path = to_posix_relative(&target_root, file)?;
        chunks.extend(chunk_file(&project_root_str, &relative_path, &content));
    }

    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start vector indexing transaction: {error}"))?;

    sqlx::query("DELETE FROM aop_vector_chunks WHERE project_root = ?")
        .bind(&project_root_str)
        .execute(&mut *transaction)
        .await
        .map_err(|error| format!("Failed to clear old vector chunks: {error}"))?;

    for chunk in &chunks {
        let vector_json = serde_json::to_string(&chunk.vector)
            .map_err(|error| format!("Failed to serialize vector embedding: {error}"))?;

        sqlx::query(
            r#"
            INSERT INTO aop_vector_chunks (
                id, project_root, file_path, start_line, end_line,
                chunk_type, name, content, vector_json, indexed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&chunk.id)
        .bind(&chunk.project_root)
        .bind(&chunk.file_path)
        .bind(chunk.start_line)
        .bind(chunk.end_line)
        .bind(&chunk.chunk_type)
        .bind(&chunk.name)
        .bind(&chunk.content)
        .bind(vector_json)
        .bind(Utc::now().timestamp())
        .execute(&mut *transaction)
        .await
        .map_err(|error| format!("Failed to insert vector chunk '{}': {error}", chunk.id))?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| format!("Failed to commit vector indexing transaction: {error}"))?;

    Ok(IndexProjectResult {
        target_project: project_root_str,
        table_name,
        indexed_files: files.len() as u32,
        indexed_chunks: chunks.len() as u32,
        index_path: "sqlite:aop_vector_chunks".to_string(),
    })
}

pub fn table_name_for_project(project_root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_root.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let suffix = digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("aop_vector_chunks_{suffix}")
}

fn normalize_project_root(target_project: &str) -> Result<PathBuf, String> {
    if target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }

    let root = PathBuf::from(target_project.trim());
    let normalized = fs::canonicalize(root)
        .map_err(|error| format!("Unable to resolve target project path: {error}"))?;

    if !normalized.is_dir() {
        return Err(format!(
            "Target project path '{}' is not a directory",
            normalized.display()
        ));
    }

    Ok(normalized)
}

fn collect_source_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut files = Vec::new();

    while let Some(current_dir) = queue.pop_front() {
        let entries = fs::read_dir(&current_dir).map_err(|error| {
            format!(
                "Failed to read directory '{}': {error}",
                current_dir.display()
            )
        })?;

        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Failed to read directory entry: {error}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("Failed to inspect '{}': {error}", path.display()))?;

            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if should_skip_dir(&name) {
                    continue;
                }
                queue.push_back(path);
                continue;
            }

            if file_type.is_file() && is_supported_extension(&path) {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".turbo"
    )
}

fn is_supported_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("ts")
            | Some("tsx")
            | Some("js")
            | Some("jsx")
            | Some("rs")
            | Some("json")
            | Some("css")
            | Some("md")
            | Some("toml")
    )
}

fn to_posix_relative(root: &Path, file: &Path) -> Result<String, String> {
    let relative = file.strip_prefix(root).map_err(|error| {
        format!(
            "Failed to compute relative path for '{}': {error}",
            file.display()
        )
    })?;

    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

fn chunk_file(project_root: &str, relative_path: &str, content: &str) -> Vec<ChunkRow> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<ChunkRow> = Vec::new();
    let mut current_start = 1usize;
    let mut current_type = "imports".to_string();
    let mut current_name = "file_scope".to_string();
    let mut buffer: Vec<String> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let line_number = idx + 1;
        if let Some((chunk_type, chunk_name)) = detect_boundary(line) {
            if !buffer.is_empty() {
                chunks.push(build_chunk(
                    project_root,
                    relative_path,
                    current_start,
                    line_number - 1,
                    &current_type,
                    &current_name,
                    &buffer.join("\n"),
                ));
                buffer.clear();
            }
            current_start = line_number;
            current_type = chunk_type;
            current_name = chunk_name;
        }

        buffer.push((*line).to_string());

        if buffer.len() >= MAX_LINES_PER_CHUNK {
            chunks.push(build_chunk(
                project_root,
                relative_path,
                current_start,
                line_number,
                &current_type,
                &current_name,
                &buffer.join("\n"),
            ));
            buffer.clear();
            current_start = line_number + 1;
        }
    }

    if !buffer.is_empty() {
        chunks.push(build_chunk(
            project_root,
            relative_path,
            current_start,
            lines.len(),
            &current_type,
            &current_name,
            &buffer.join("\n"),
        ));
    }

    chunks
}

fn detect_boundary(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();

    if trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("async fn ")
    {
        return Some((
            "function".to_string(),
            extract_name_after_keywords(trimmed, &["fn"]),
        ));
    }
    if trimmed.starts_with("function ") || trimmed.starts_with("export function ") {
        return Some((
            "function".to_string(),
            extract_name_after_keywords(trimmed, &["function"]),
        ));
    }
    if trimmed.starts_with("const ") && trimmed.contains("=>") {
        return Some((
            "function".to_string(),
            extract_name_after_keywords(trimmed, &["const"]),
        ));
    }
    if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
        return Some((
            "class".to_string(),
            extract_name_after_keywords(trimmed, &["class"]),
        ));
    }
    if trimmed.starts_with("interface ")
        || trimmed.starts_with("export interface ")
        || trimmed.starts_with("type ")
        || trimmed.starts_with("export type ")
        || trimmed.starts_with("enum ")
        || trimmed.starts_with("export enum ")
    {
        return Some((
            "type".to_string(),
            extract_name_after_keywords(trimmed, &["interface", "type", "enum"]),
        ));
    }
    if trimmed.starts_with("export ") || trimmed.starts_with("module.exports") {
        return Some(("export".to_string(), "export_block".to_string()));
    }

    None
}

fn extract_name_after_keywords(line: &str, keywords: &[&str]) -> String {
    let tokens = line
        .split(|ch: char| {
            ch.is_whitespace() || matches!(ch, '(' | '{' | ':' | '=' | ';' | '<' | '>')
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    for (idx, token) in tokens.iter().enumerate() {
        if keywords.iter().any(|keyword| keyword == token) {
            if let Some(name) = tokens.get(idx + 1) {
                return (*name).to_string();
            }
        }
    }

    "anonymous".to_string()
}

fn build_chunk(
    project_root: &str,
    relative_path: &str,
    start_line: usize,
    end_line: usize,
    chunk_type: &str,
    name: &str,
    content: &str,
) -> ChunkRow {
    ChunkRow {
        id: Uuid::new_v4().to_string(),
        project_root: project_root.to_string(),
        file_path: relative_path.to_string(),
        start_line: start_line as i64,
        end_line: end_line as i64,
        chunk_type: chunk_type.to_string(),
        name: name.to_string(),
        content: content.to_string(),
        vector: embed_text(content),
    }
}

pub fn embed_text(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0_f32; VECTOR_DIM];

    for token in text
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .filter(|token| token.len() >= 3)
    {
        let token = token.to_ascii_lowercase();
        let digest = Sha256::digest(token.as_bytes());
        let idx = u16::from_le_bytes([digest[0], digest[1]]) as usize % VECTOR_DIM;
        let sign = if digest[2] % 2 == 0 { 1.0 } else { -1.0 };
        vector[idx] += sign;
    }

    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }

    vector
}
