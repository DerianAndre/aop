CREATE TABLE IF NOT EXISTS aop_budget_requests (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES aop_tasks(id) ON DELETE CASCADE,
    requested_by TEXT NOT NULL,
    reason TEXT NOT NULL,
    requested_increment INTEGER NOT NULL,
    current_budget INTEGER NOT NULL,
    current_usage INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected')),
    approved_increment INTEGER,
    resolution_note TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    resolved_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_budget_requests_task ON aop_budget_requests(task_id);
CREATE INDEX IF NOT EXISTS idx_budget_requests_status ON aop_budget_requests(status);
CREATE INDEX IF NOT EXISTS idx_budget_requests_created_at ON aop_budget_requests(created_at);
