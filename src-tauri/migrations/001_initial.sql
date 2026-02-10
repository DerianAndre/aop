CREATE TABLE aop_tasks (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES aop_tasks(id),
    tier INTEGER NOT NULL CHECK (tier IN (1, 2, 3)),
    domain TEXT NOT NULL,
    objective TEXT NOT NULL,
    status TEXT DEFAULT 'pending'
        CHECK (status IN ('pending', 'executing', 'completed', 'failed', 'paused')),
    token_budget INTEGER NOT NULL,
    token_usage INTEGER DEFAULT 0,
    context_efficiency_ratio REAL DEFAULT 0.0,
    risk_factor REAL DEFAULT 0.0,
    compliance_score INTEGER DEFAULT 0,
    checksum_before TEXT,
    checksum_after TEXT,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_tasks_parent ON aop_tasks(parent_id);
CREATE INDEX idx_tasks_status ON aop_tasks(status);
CREATE INDEX idx_tasks_tier ON aop_tasks(tier);

CREATE TABLE aop_mutations (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES aop_tasks(id),
    agent_uid TEXT NOT NULL,
    file_path TEXT NOT NULL,
    diff_content TEXT NOT NULL,
    intent_description TEXT,
    intent_hash TEXT,
    confidence REAL DEFAULT 0.0,
    test_result TEXT,
    test_exit_code INTEGER,
    rejection_reason TEXT,
    rejected_at_step TEXT,
    status TEXT DEFAULT 'proposed'
        CHECK (status IN ('proposed', 'validated', 'validated_no_tests', 'applied', 'rejected')),
    proposed_at INTEGER NOT NULL,
    applied_at INTEGER
);

CREATE INDEX idx_mutations_task ON aop_mutations(task_id);
CREATE INDEX idx_mutations_status ON aop_mutations(status);
CREATE INDEX idx_mutations_agent ON aop_mutations(agent_uid);

CREATE TABLE agent_metrics (
    agent_uid TEXT PRIMARY KEY,
    persona TEXT NOT NULL,
    model_name TEXT,
    total_calls INTEGER DEFAULT 0,
    successful_proposals INTEGER DEFAULT 0,
    rejected_proposals INTEGER DEFAULT 0,
    success_rate REAL DEFAULT 0.0,
    avg_tid REAL DEFAULT 0.0,
    total_tokens_spent INTEGER DEFAULT 0,
    last_active INTEGER
);

CREATE TABLE aop_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target_id TEXT,
    details TEXT
);

CREATE INDEX idx_audit_timestamp ON aop_audit_log(timestamp);
CREATE INDEX idx_audit_actor ON aop_audit_log(actor);
