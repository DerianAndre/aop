CREATE TABLE IF NOT EXISTS aop_vector_chunks (
    id TEXT PRIMARY KEY,
    project_root TEXT NOT NULL,
    file_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    chunk_type TEXT NOT NULL,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    vector_json TEXT NOT NULL,
    indexed_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_vector_chunks_project ON aop_vector_chunks(project_root);
CREATE INDEX IF NOT EXISTS idx_vector_chunks_file ON aop_vector_chunks(file_path);
