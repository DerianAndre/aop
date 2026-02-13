CREATE TABLE IF NOT EXISTS aop_agent_runs (
    id TEXT PRIMARY KEY,
    root_task_id TEXT,
    task_id TEXT,
    tier INTEGER,
    actor TEXT NOT NULL,
    persona TEXT,
    skill TEXT,
    provider TEXT,
    model_id TEXT,
    adapter_kind TEXT,
    status TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,
    token_delta INTEGER DEFAULT 0,
    cost_usd REAL,
    metadata_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_agent_runs_root_task ON aop_agent_runs(root_task_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_task ON aop_agent_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_actor ON aop_agent_runs(actor);
CREATE INDEX IF NOT EXISTS idx_agent_runs_status ON aop_agent_runs(status);
CREATE INDEX IF NOT EXISTS idx_agent_runs_started_at ON aop_agent_runs(started_at);

CREATE TABLE IF NOT EXISTS aop_agent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT,
    root_task_id TEXT,
    task_id TEXT,
    tier INTEGER,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    status TEXT,
    phase TEXT,
    message TEXT,
    provider TEXT,
    model_id TEXT,
    persona TEXT,
    skill TEXT,
    mcp_server TEXT,
    mcp_tool TEXT,
    latency_ms INTEGER,
    retry_count INTEGER,
    tokens_in INTEGER,
    tokens_out INTEGER,
    token_delta INTEGER,
    cost_usd REAL,
    payload_json TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_events_run ON aop_agent_events(run_id);
CREATE INDEX IF NOT EXISTS idx_agent_events_root_task ON aop_agent_events(root_task_id);
CREATE INDEX IF NOT EXISTS idx_agent_events_task ON aop_agent_events(task_id);
CREATE INDEX IF NOT EXISTS idx_agent_events_actor ON aop_agent_events(actor);
CREATE INDEX IF NOT EXISTS idx_agent_events_action ON aop_agent_events(action);
CREATE INDEX IF NOT EXISTS idx_agent_events_created_at ON aop_agent_events(created_at);

CREATE TABLE IF NOT EXISTS aop_model_health (
    provider TEXT NOT NULL,
    model_id TEXT NOT NULL,
    total_calls INTEGER DEFAULT 0,
    success_calls INTEGER DEFAULT 0,
    failed_calls INTEGER DEFAULT 0,
    avg_latency_ms REAL DEFAULT 0,
    avg_cost_usd REAL DEFAULT 0,
    quality_score REAL DEFAULT 0.70,
    last_error TEXT,
    last_used_at INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(provider, model_id)
);

CREATE INDEX IF NOT EXISTS idx_model_health_updated_at ON aop_model_health(updated_at);
