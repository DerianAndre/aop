use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use sqlx::SqlitePool;

use crate::vector::ContextChunk;

use super::indexer::embed_text;

#[derive(Debug, Clone, sqlx::FromRow)]
struct StoredChunk {
    id: String,
    file_path: String,
    start_line: i64,
    end_line: i64,
    chunk_type: String,
    name: String,
    content: String,
    vector_json: String,
}

pub async fn query_codebase(
    pool: &SqlitePool,
    target_project: &str,
    query: &str,
    top_k: u32,
) -> Result<Vec<ContextChunk>, String> {
    if query.trim().is_empty() {
        return Err("query is required".to_string());
    }

    let project_root = normalize_project_root(target_project)?;
    let project_root_str = project_root.to_string_lossy().to_string();
    let query_vector = embed_text(query);
    let limit = usize::try_from(top_k.max(1)).unwrap_or(5);

    let rows = sqlx::query_as::<_, StoredChunk>(
        r#"
        SELECT id, file_path, start_line, end_line, chunk_type, name, content, vector_json
        FROM aop_vector_chunks
        WHERE project_root = ?
        "#,
    )
    .bind(&project_root_str)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to query vector chunks from SQLite: {error}"))?;

    let mut scored = Vec::new();
    for row in rows {
        let vector = serde_json::from_str::<Vec<f32>>(&row.vector_json).map_err(|error| {
            format!(
                "Failed to decode stored embedding for chunk '{}': {error}",
                row.id
            )
        })?;

        let score = cosine_similarity(&query_vector, &vector);
        scored.push(ContextChunk {
            id: row.id,
            file_path: row.file_path,
            start_line: row.start_line.max(0) as u32,
            end_line: row.end_line.max(0) as u32,
            chunk_type: row.chunk_type,
            name: row.name,
            content: row.content,
            score,
        });
    }

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    scored.truncate(limit);

    Ok(scored)
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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let a_norm = a.iter().map(|value| value * value).sum::<f32>().sqrt();
    let b_norm = b.iter().map(|value| value * value).sum::<f32>().sqrt();

    if a_norm == 0.0 || b_norm == 0.0 {
        return 0.0;
    }

    dot / (a_norm * b_norm)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::db;
    use crate::vector::indexer::index_project;

    use super::query_codebase;

    #[tokio::test]
    async fn indexes_and_returns_semantic_chunks() {
        let project_temp = tempdir().expect("project temp dir should exist");
        let src_dir = project_temp.path().join("src");
        std::fs::create_dir_all(&src_dir).expect("src directory should be created");
        std::fs::write(
            src_dir.join("session.ts"),
            "export function useSession() {\n  return { loading: false }\n}\n",
        )
        .expect("fixture should be written");

        let db_dir = tempdir().expect("db temp dir should exist");
        let db_path: PathBuf = db_dir.path().join("vector-test.db");
        let pool = db::connect_pool(&db_path)
            .await
            .expect("sqlite pool should initialize");
        db::run_migrations(&pool)
            .await
            .expect("migrations should initialize");

        let indexed = index_project(&pool, &project_temp.path().to_string_lossy())
            .await
            .expect("indexing should succeed");
        assert!(indexed.indexed_files >= 1);
        assert!(indexed.indexed_chunks >= 1);

        let chunks = query_codebase(
            &pool,
            &project_temp.path().to_string_lossy(),
            "session loading state",
            5,
        )
        .await
        .expect("query should succeed");

        assert!(!chunks.is_empty());
        assert!(chunks
            .iter()
            .any(|chunk| chunk.file_path.ends_with("session.ts")));
    }
}
