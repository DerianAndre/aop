# Autonomous Orchestration Platform (AOP)

**Role**: Lead Systems Architect & Senior Platform Engineer.

**Objective**: Build AOP, a standalone desktop application that governs, repairs, and optimizes external codebases through a multi-tier agent system. AOP maintains its own state (database, vector index, logs) completely separate from the target project. It reads, analyzes, proposes diffs, tests them in isolation, and only writes to the target after validation passes. Zero-trust: every mutation is verified before it touches the codebase.

**Core Principle**: AOP never directly modifies the target codebase. All changes flow through a validated pipeline: propose → test → approve → apply.

---

## Table of Contents

1. [Technology Stack](#1-technology-stack)
2. [Project Structure](#2-project-structure)
3. [Implementation Phases](#3-implementation-phases)
4. [Agent System Architecture](#4-agent-system-architecture)
5. [Inter-Tier Communication Contracts](#5-inter-tier-communication-contracts)
6. [Task State Machine](#6-task-state-machine)
7. [Context & Token Management](#7-context--token-management)
8. [Diff Pipeline (Mutation Rail)](#8-diff-pipeline)
9. [Shadow Testing](#9-shadow-testing)
10. [Database Schema](#10-database-schema)
11. [Vector Index Engine](#11-vector-index-engine)
12. [Data Integrity Rules](#12-data-integrity-rules)
13. [UI Dashboard](#13-ui-dashboard)
14. [Error Handling & Recovery](#14-error-handling--recovery)
15. [Formulas & Thresholds](#15-formulas--thresholds)

---

## 1. Technology Stack

### Pinned Dependencies

**Backend (Tauri/Rust)**

```toml
# Cargo.toml
[dependencies]
tauri = { version = "2.10", features = ["shell-open"] }
sqlx = { version = "0.8.6", features = ["runtime-tokio", "sqlite"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1.18", features = ["v4"] }
sha2 = "0.10.9"            # For diff hashing / attribution
chrono = "0.4.42"
lancedb = "0.23"            # Vector DB - local, embedded, no server needed
arrow = "57.2"
```

**Frontend (React)**

```json
{
  "dependencies": {
    "react": "^19.2.4",
    "react-dom": "^19.2.4",
    "@xyflow/react": "^12.10.0",
    "tailwindcss": "^4.1.18",
    "@tauri-apps/api": "^2.10.1",
    "@tanstack/react-query": "^5.90.20",
    "zustand": "^5.0.11",
    "recharts": "^3.7.0",
    "tailwind-merge": "^3.4.0",
    "clsx": "^2.1.1",
    "tw-animate-css": "^1.4.0",
    "lucide-react": "^0.563.0"
  }
}
```

**MCP Bridge (Node.js/TypeScript)**

```json
{
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.26.0",
    "typescript": "^5.7.0"
  }
}
```

**shadcn/ui Setup (Tailwind v4 + OKLCH)**

```ts
// src/lib/utils.ts — cn() helper
import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

```css
/* src/styles/globals.css */
@import "tailwindcss";
@import "tw-animate-css";

@custom-variant dark (&:is(.dark *));

:root {
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);
  --card: oklch(1 0 0);
  --card-foreground: oklch(0.145 0 0);
  --popover: oklch(1 0 0);
  --popover-foreground: oklch(0.145 0 0);
  --primary: oklch(0.205 0 0);
  --primary-foreground: oklch(0.985 0 0);
  --secondary: oklch(0.97 0 0);
  --secondary-foreground: oklch(0.205 0 0);
  --muted: oklch(0.97 0 0);
  --muted-foreground: oklch(0.556 0 0);
  --accent: oklch(0.97 0 0);
  --accent-foreground: oklch(0.205 0 0);
  --destructive: oklch(0.577 0.245 27.325);
  --destructive-foreground: oklch(0.985 0 0);
  --border: oklch(0.922 0 0);
  --input: oklch(0.922 0 0);
  --ring: oklch(0.708 0 0);
  --radius: 0.625rem;
}

.dark {
  --background: oklch(0.145 0 0);
  --foreground: oklch(0.985 0 0);
  --card: oklch(0.145 0 0);
  --card-foreground: oklch(0.985 0 0);
  --popover: oklch(0.145 0 0);
  --popover-foreground: oklch(0.985 0 0);
  --primary: oklch(0.985 0 0);
  --primary-foreground: oklch(0.205 0 0);
  --secondary: oklch(0.269 0 0);
  --secondary-foreground: oklch(0.985 0 0);
  --muted: oklch(0.269 0 0);
  --muted-foreground: oklch(0.708 0 0);
  --accent: oklch(0.269 0 0);
  --accent-foreground: oklch(0.985 0 0);
  --destructive: oklch(0.577 0.245 27.325);
  --destructive-foreground: oklch(0.985 0 0);
  --border: oklch(0.269 0 0);
  --input: oklch(0.269 0 0);
  --ring: oklch(0.439 0 0);
}

@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-destructive-foreground: var(--destructive-foreground);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --radius-sm: calc(var(--radius) - 4px);
  --radius-md: calc(var(--radius) - 2px);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) + 4px);
}
```

```jsonc
// components.json (root del proyecto)
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/styles/globals.css",
    "baseColor": "neutral",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  },
  "iconLibrary": "lucide"
}
```

> **Agregar componentes**: `npx shadcn@latest add button card dialog badge tabs scroll-area`
> Cada componente se copia a `src/components/ui/` — sin abstracciones, código tuyo.

### Stack Justification (brief)

| Component | Choice | Why |
|---|---|---|
| Desktop shell | Tauri 2 (Rust) | Memory-safe, fast IPC, small binary |
| Database | SQLite via SQLx | Embedded, zero-config, ACID compliant |
| Vector DB | LanceDB | Embedded (no server), fast similarity search |
| Frontend | React 19 + Tailwind 4 | Component model fits task tree visualization |
| UI Components | shadcn/ui (Tailwind v4) | Copy-paste components, no abstraction, OKLCH theming |
| Graph viz | React Flow (@xyflow) | Purpose-built for node graphs, zoom/pan |
| MCP comms | Node.js bridge | Async tool calling, multiplexing agent requests |
| State mgmt | Zustand | Simple, no boilerplate, works with React 19 |

---

## 2. Project Structure

```
aop/
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   ├── tasks.rs          # CRUD for aop_tasks
│   │   │   ├── mutations.rs      # CRUD for aop_mutations
│   │   │   └── metrics.rs        # Agent performance tracking
│   │   ├── agents/
│   │   │   ├── mod.rs
│   │   │   ├── orchestrator.rs   # Tier 1 logic
│   │   │   ├── domain_leader.rs  # Tier 2 logic
│   │   │   ├── specialist.rs     # Tier 3 logic
│   │   │   └── prompts/          # System prompts per agent type
│   │   ├── mcp_bridge/
│   │   │   ├── mod.rs
│   │   │   ├── client.rs         # MCP client connection
│   │   │   └── tool_caller.rs    # Multiplexed tool calls
│   │   ├── diff_pipeline/
│   │   │   ├── mod.rs
│   │   │   ├── proposer.rs
│   │   │   ├── validator.rs
│   │   │   └── applier.rs
│   │   ├── vector/
│   │   │   ├── mod.rs
│   │   │   ├── indexer.rs        # AST-aware chunking + embedding
│   │   │   └── search.rs         # Similarity queries
│   │   └── commands.rs           # Tauri IPC commands
│   ├── migrations/
│   │   └── 001_initial.sql
│   └── Cargo.toml
├── src/                          # React frontend
│   ├── App.tsx
│   ├── styles/
│   │   └── globals.css           # Tailwind v4 + shadcn/ui theme (OKLCH)
│   ├── lib/
│   │   └── utils.ts              # cn() helper (clsx + tailwind-merge)
│   ├── components/
│   │   ├── ui/                   # shadcn/ui components (copy-paste)
│   │   │   ├── button.tsx
│   │   │   ├── card.tsx
│   │   │   ├── dialog.tsx
│   │   │   ├── badge.tsx
│   │   │   ├── tabs.tsx
│   │   │   ├── scroll-area.tsx
│   │   │   └── ...               # Add as needed via: npx shadcn@latest add <component>
│   │   ├── TaskGraph.tsx         # React Flow visualization
│   │   ├── DiffReviewer.tsx      # Side-by-side diff view
│   │   ├── TokenBurnChart.tsx    # Recharts dashboard
│   │   └── ConflictResolver.tsx  # Human escalation UI
│   ├── stores/
│   │   └── taskStore.ts          # Zustand store
│   ├── hooks/
│   │   └── useTauri.ts           # IPC wrappers
│   └── types/
│       └── index.ts              # Shared TypeScript interfaces
├── mcp-bridge/                   # Node.js MCP bridge
│   ├── src/
│   │   ├── index.ts
│   │   ├── bridge.ts             # MCP client + multiplexer
│   │   └── tools.ts              # Tool definitions
│   ├── package.json
│   └── tsconfig.json
├── aop_orchestrator.db           # SQLite (auto-created)
├── components.json               # shadcn/ui config
└── package.json
```

---

## 3. Implementation Phases

Each phase is a self-contained deliverable. Do NOT skip phases or start a later phase before the current one compiles and runs.

### Phase 1: Foundation (Tauri + SQLite + Basic UI)

**Goal**: Desktop app opens, database exists, you can create/view tasks manually.

1. Initialize Tauri 2 project with React 19 frontend
2. Create SQLite database with full schema (see Section 10)
3. Implement Tauri IPC commands: `create_task`, `get_tasks`, `update_task_status`
4. Build basic React UI: task list view with status badges
5. **Done when**: You can create a task from the UI and see it persisted in SQLite

### Phase 2: MCP Bridge (Read Target Codebase)

**Goal**: AOP can connect to a target project's MCP server and read files.

1. Build Node.js MCP bridge that connects to a target MCP server (e.g., AIDD)
2. Implement tool calling: `read_file`, `list_dir`, `search_files`
3. Wire bridge to Tauri backend via child process or sidecar
4. Expose `read_target_file` and `list_target_dir` as Tauri commands
5. **Done when**: You can browse the target project's file tree from AOP's UI

### Phase 3: Vector Index Engine

**Goal**: AOP can index a target codebase and answer semantic queries.

1. Integrate LanceDB (embedded, no server)
2. Implement AST-aware chunking: parse files into functions/classes/blocks, not raw lines
3. Generate embeddings for each chunk (use a local model or API)
4. Implement similarity search: `query_codebase("components using useSession without loading state")`
5. **Done when**: Semantic search returns relevant code blocks for natural language queries

### Phase 4: Agent System (Tier 1 Only)

**Goal**: A single orchestrator agent can decompose an objective into sub-tasks.

1. Define what an agent IS technically (see Section 4)
2. Implement Tier 1 orchestrator: takes a natural language objective, produces a list of `TaskAssignment` objects
3. Persist generated tasks to SQLite with parent-child relationships
4. Calculate risk scores using the PRA formula (see Section 15)
5. Allocate token budgets per sub-task
6. **Done when**: Given "Refactor auth module", Tier 1 creates 3-5 concrete sub-tasks in the DB

### Phase 5: Agent System (Tiers 2 & 3)

**Goal**: Full agent hierarchy can analyze code and propose diffs.

1. Implement Tier 2 (domain leader): receives a task, reads relevant code via MCP + vector search, spawns Tier 3 specialists
2. Implement Tier 3 (specialist): reads specific files, produces a `DiffProposal`
3. Implement consensus protocol: if two Tier 3 agents disagree (semantic distance > 0.3), spawn arbitrator
4. Implement context compression: Tier 2 summarizes Tier 3 output before reporting to Tier 1
5. **Done when**: Given a sub-task, the system produces a concrete diff proposal with agent attribution

### Phase 6: Diff Pipeline (Propose → Test → Apply)

**Goal**: Diffs go through the full validation pipeline before touching the target.

1. Implement diff proposal storage in `aop_mutations`
2. Implement shadow testing (see Section 9)
3. Implement semantic regression check: compare intent embeddings before/after
4. Implement Tier 2 architectural compliance check
5. Implement Tier 1 final approval
6. Implement atomic apply: write to target via MCP, commit via git
7. **Done when**: A diff flows from proposal to applied with full audit trail

### Phase 7: UI Dashboard

**Goal**: Full visualization of the agent system's activity.

1. Task graph with React Flow: nodes = tasks, edges = parent-child, colors = status
2. Token burn chart: real-time line chart of tokens spent vs compliance gained
3. Diff reviewer: side-by-side view with "why this change" summary
4. Conflict resolution UI: when consensus fails, present options to user
5. **Done when**: User can monitor and intervene in the entire pipeline from the UI

---

## 4. Agent System Architecture

### What IS an Agent (Technical Definition)

An agent is **a single LLM API call** with a specific system prompt, a set of allowed tools, and a token budget. It is NOT a long-running process, NOT a thread, NOT a container. It's a stateless function call.

```typescript
interface AgentCall {
  agentUid: string;           // Unique ID for attribution (UUID v4)
  tier: 1 | 2 | 3;
  persona: string;            // e.g., "security_analyst", "react_specialist"
  systemPrompt: string;       // Role-specific instructions
  allowedTools: string[];     // What MCP tools this agent can use
  tokenBudget: number;        // Max tokens for this call
  context: AgentContext;      // Pre-hydrated code snippets + task info
}

interface AgentContext {
  taskObjective: string;
  parentSummary?: string;     // Compressed summary from parent tier
  codeSnippets: CodeBlock[];  // Pre-fetched relevant code
  constraints: string[];      // Architectural rules from ADR.md
}

interface CodeBlock {
  filePath: string;
  startLine: number;
  endLine: number;
  content: string;
  embedding?: number[];       // Vector for similarity comparison
}
```

### Agent Lifecycle

```
1. Tier 1 receives objective from user
2. Tier 1 makes ONE LLM call → returns list of TaskAssignments
3. For each TaskAssignment, Tier 2 is invoked:
   a. Tier 2 queries vector DB for relevant code
   b. Tier 2 makes ONE LLM call → decides how to split work
   c. Tier 2 spawns Tier 3 calls (1-3 specialists)
4. Each Tier 3 makes ONE LLM call → returns a DiffProposal
5. Tier 2 collects proposals, runs consensus if needed
6. Tier 2 compresses results → sends IntentSummary to Tier 1
7. Tier 1 reviews and approves/rejects
```

### Tier Permissions

| Tier | Can Read Files | Can Propose Diffs | Can Write Files | Can Approve | Can Spawn Sub-agents |
|---|---|---|---|---|---|
| 1 (Orchestrator) | Via summary only | No | Yes (after approval) | Yes | Tier 2 only |
| 2 (Domain Leader) | Yes (via MCP) | No | No | Tier 3 proposals | Tier 3 only |
| 3 (Specialist) | Yes (via MCP) | Yes | No | No | No |

### Specialist Personas (Tier 3)

Each persona is a system prompt template stored in `src-tauri/src/agents/prompts/`:

- `security_analyst.md` — Focus: vulnerabilities, auth patterns, input validation
- `react_specialist.md` — Focus: component architecture, hooks, rendering
- `database_optimizer.md` — Focus: queries, indexes, schema design
- `test_engineer.md` — Focus: test coverage, edge cases, assertions
- `style_enforcer.md` — Focus: linting, naming conventions, code consistency

---

## 5. Inter-Tier Communication Contracts

All communication between tiers uses typed messages. No free-form text between tiers.

### User → Tier 1

```typescript
interface UserObjective {
  id: string;
  objective: string;        // Natural language: "Refactor auth for performance"
  targetProject: string;    // MCP server identifier
  globalTokenBudget: number;
  maxRiskTolerance: number; // 0.0 to 1.0
}
```

### Tier 1 → Tier 2

```typescript
interface TaskAssignment {
  taskId: string;
  parentId: string;
  tier: 2;
  domain: string;            // e.g., "auth", "database", "frontend"
  objective: string;         // Specific: "Optimize useSession hook re-renders"
  tokenBudget: number;
  riskFactor: number;        // Calculated by PRA formula
  constraints: string[];     // From ADR.md: ["no-direct-db-calls-from-components"]
  relevantFiles: string[];   // Pre-identified by vector search
}
```

### Tier 2 → Tier 3

```typescript
interface SpecialistTask {
  taskId: string;
  parentId: string;
  tier: 3;
  persona: string;           // "react_specialist"
  objective: string;         // Atomic: "Add loading state to SessionProvider"
  tokenBudget: number;
  targetFiles: string[];     // Exact files to read
  codeContext: CodeBlock[];   // Pre-hydrated code snippets
  constraints: string[];
}
```

### Tier 3 → Tier 2

```typescript
interface DiffProposal {
  proposalId: string;
  taskId: string;
  agentUid: string;
  filePath: string;
  diffContent: string;       // Unified diff format
  intentDescription: string; // "Added loading state check before rendering session data"
  intentHash: string;        // SHA-256 of intent embedding vector
  confidence: number;        // 0.0 to 1.0
  tokensUsed: number;
}
```

### Tier 2 → Tier 1

```typescript
interface IntentSummary {
  taskId: string;
  domain: string;
  status: 'ready_for_review' | 'consensus_failed' | 'blocked';
  proposals: DiffProposal[];
  complianceScore: number;    // 0 to 100
  tokensSpent: number;
  summary: string;            // 2-3 sentence compressed summary
  conflicts?: ConflictReport; // Only if consensus failed
}

interface ConflictReport {
  agentA: string;
  agentB: string;
  semanticDistance: number;
  description: string;
  requiresHumanReview: boolean;
}
```

---

## 6. Task State Machine

Every task follows this exact state machine. No exceptions.

```
                    ┌──────────────┐
                    │   pending    │
                    └──────┬───────┘
                           │ Agent picks up task
                           ▼
                    ┌──────────────┐
              ┌─────│  executing   │─────┐
              │     └──────┬───────┘     │
              │            │             │
       tokens > budget   success    unrecoverable error
              │            │             │
              ▼            ▼             ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │  paused  │ │completed │ │  failed  │
        └────┬─────┘ └──────────┘ └──────────┘
             │
      human resumes / budget increased
             │
             ▼
        back to executing
```

### Transition Rules

| From | To | Condition |
|---|---|---|
| `pending` | `executing` | Agent assigned and first LLM call initiated |
| `executing` | `completed` | All proposals validated + CI passes + compliance_score >= 70 |
| `executing` | `failed` | CI fails after 2 retries OR agent returns error 3 times |
| `executing` | `paused` | token_usage > token_budget * 0.85 (85% threshold) |
| `paused` | `executing` | Human approves budget increase OR provides guidance |
| `failed` | `pending` | Human requests retry with modified constraints |

### Rules

- A task can only be `completed` if ALL its child tasks are `completed`
- A parent task auto-transitions to `failed` if >50% of children are `failed`
- `paused` tasks are surfaced to the user in the UI with context about why

---

## 7. Context & Token Management

The context window is a finite, expensive resource. Every agent call must be efficient.

### Token Efficiency Formula

```
TID = (compliance_gain / tokens_spent) * 100
```

Where:
- `compliance_gain` = compliance_score_after - compliance_score_before (0-100 scale)
- `tokens_spent` = total input + output tokens for this task

A TID below 0.5 means the agent is burning tokens without producing value. Tier 1 should cut budget or reassign.

### Context Hydration Strategy (for Tier 3 agents)

Tier 3 agents do NOT get the entire codebase. They get pre-hydrated context:

```
Step 1 — Peek:
  Read file headers + export lists only (low token cost)
  → Identify which files are relevant

Step 2 — Vector Search:
  Query LanceDB: "functions that call useSession"
  → Get ranked list of code blocks with similarity scores

Step 3 — Hydrate:
  Fetch ONLY the specific functions/blocks needed
  → Assemble minimal context for the LLM call
```

### Token Budget Allocation

Tier 1 distributes the global budget like this:

```
Global Budget: 100%
├── Tier 1 overhead: 10% (decomposition + final review)
├── Reserve: 10% (for retries and consensus arbitration)
└── Distributed to Tier 2 tasks: 80%
    └── Each Tier 2 distributes to its Tier 3 specialists
```

### Context Pause Threshold

When an agent's `token_usage` exceeds **85%** of its `token_budget`:

1. Agent must STOP execution
2. Save current findings to `aop_tasks.context_efficiency_ratio`
3. Transition task to `paused`
4. Surface to user with summary of what was found so far

---

## 8. Diff Pipeline

The diff pipeline is the ONLY path for changes to reach the target codebase.

```
┌─────────────┐     ┌─────────────┐     ┌──────────────────┐
│ Tier 3       │────▶│ Store in    │────▶│ Shadow Test      │
│ proposes diff│     │ aop_mutations│     │ (isolated clone) │
└─────────────┘     └─────────────┘     └────────┬─────────┘
                                                  │
                                          CI passes?
                                         ╱          ╲
                                       Yes           No
                                        │             │
                                        ▼             ▼
                              ┌──────────────┐  ┌──────────┐
                              │ Semantic      │  │ Rejected │
                              │ Regression   │  │ (log why)│
                              │ Check        │  └──────────┘
                              └──────┬───────┘
                                     │
                              Intent preserved?
                               ╱          ╲
                             Yes           No
                              │             │
                              ▼             ▼
                    ┌──────────────┐  ┌──────────┐
                    │ Tier 2       │  │ Rejected │
                    │ Compliance   │  │ (log why)│
                    │ Check        │  └──────────┘
                    └──────┬───────┘
                           │
                    Compliant?
                     ╱       ╲
                   Yes        No
                    │          │
                    ▼          ▼
          ┌──────────────┐  ┌──────────┐
          │ Tier 1       │  │ Rejected │
          │ Final Review │  │ (log why)│
          └──────┬───────┘  └──────────┘
                 │
                 ▼
          ┌──────────────┐
          │ Apply to     │
          │ target via   │
          │ MCP + git    │
          └──────────────┘
```

### Mutation Statuses

```
proposed → validated → applied
proposed → rejected (at any validation step)
```

Every rejection is logged with:
- Which step rejected it
- Why (CI error, semantic regression, compliance violation)
- The agent that proposed it (for metrics)

---

## 9. Shadow Testing

Shadow testing runs the proposed diff in isolation before it touches the real codebase.

### How It Works (Concrete Implementation)

```
1. Clone target project to a temporary directory:
   - On Linux/Mac: tmpfs mount (RAM-based, fast, auto-cleanup)
   - On Windows: %TEMP%\aop_shadow_<uuid>\ directory
   - Alternative: Docker container if available

2. Apply the diff:
   - Use `git apply --check` first (dry run)
   - If dry run passes, `git apply` the diff

3. Run CI:
   - Execute the target project's test command via MCP
   - Tool: `aidd_ci_report` or project-defined test script
   - Capture: exit code, stdout, stderr, duration

4. Collect results:
   - Pass: CI exit code 0, all tests green
   - Fail: Store full error output in aop_mutations.test_result

5. Cleanup:
   - Delete temporary directory
   - On failure: keep for 30 minutes for debugging, then auto-delete
```

### Fallback if No CI Exists

If the target project has no test suite:
1. Static analysis only (lint, type-check)
2. Flag the mutation as `validated_no_tests` (separate status)
3. Surface to user with warning: "No automated tests available"

---

## 10. Database Schema

File: `migrations/001_initial.sql`

```sql
-- Hierarchical Task Tree
CREATE TABLE aop_tasks (
    id TEXT PRIMARY KEY,                        -- UUID v4
    parent_id TEXT REFERENCES aop_tasks(id),    -- NULL for root (Tier 1) tasks
    tier INTEGER NOT NULL CHECK (tier IN (1, 2, 3)),
    domain TEXT NOT NULL,                       -- e.g., "auth", "database"
    objective TEXT NOT NULL,                    -- What this task should accomplish
    status TEXT DEFAULT 'pending'
        CHECK (status IN ('pending', 'executing', 'completed', 'failed', 'paused')),
    token_budget INTEGER NOT NULL,              -- Max tokens allocated
    token_usage INTEGER DEFAULT 0,              -- Tokens consumed so far
    context_efficiency_ratio REAL DEFAULT 0.0,  -- TID metric
    risk_factor REAL DEFAULT 0.0,               -- PRA result (0.0 to 1.0)
    compliance_score INTEGER DEFAULT 0,         -- 0 to 100
    checksum_before TEXT,                       -- SHA-256 of target files before
    checksum_after TEXT,                        -- SHA-256 of target files after
    error_message TEXT,                         -- Why it failed (if failed)
    retry_count INTEGER DEFAULT 0,              -- How many times retried
    created_at INTEGER NOT NULL,                -- Unix timestamp (seconds)
    updated_at INTEGER NOT NULL                 -- Unix timestamp (seconds)
);

CREATE INDEX idx_tasks_parent ON aop_tasks(parent_id);
CREATE INDEX idx_tasks_status ON aop_tasks(status);
CREATE INDEX idx_tasks_tier ON aop_tasks(tier);

-- Mutation Proposals (Diffs)
CREATE TABLE aop_mutations (
    id TEXT PRIMARY KEY,                        -- UUID v4
    task_id TEXT NOT NULL REFERENCES aop_tasks(id),
    agent_uid TEXT NOT NULL,                    -- Which agent proposed this
    file_path TEXT NOT NULL,                    -- Target file
    diff_content TEXT NOT NULL,                 -- Unified diff format
    intent_description TEXT,                    -- Human-readable intent
    intent_hash TEXT,                           -- SHA-256 of intent embedding
    confidence REAL DEFAULT 0.0,                -- Agent's confidence (0.0 to 1.0)
    test_result TEXT,                           -- Full CI output (stdout + stderr)
    test_exit_code INTEGER,                     -- CI exit code (0 = pass)
    rejection_reason TEXT,                      -- Why rejected (if rejected)
    rejected_at_step TEXT,                      -- "shadow_test" | "semantic_check" | "compliance" | "tier1_review"
    status TEXT DEFAULT 'proposed'
        CHECK (status IN ('proposed', 'validated', 'validated_no_tests', 'applied', 'rejected')),
    proposed_at INTEGER NOT NULL,               -- Unix timestamp
    applied_at INTEGER                          -- Unix timestamp (NULL until applied)
);

CREATE INDEX idx_mutations_task ON aop_mutations(task_id);
CREATE INDEX idx_mutations_status ON aop_mutations(status);
CREATE INDEX idx_mutations_agent ON aop_mutations(agent_uid);

-- Agent Performance Tracking
CREATE TABLE agent_metrics (
    agent_uid TEXT PRIMARY KEY,
    persona TEXT NOT NULL,                      -- e.g., "react_specialist"
    model_name TEXT,                            -- e.g., "claude-sonnet-4-5-20250514"
    total_calls INTEGER DEFAULT 0,
    successful_proposals INTEGER DEFAULT 0,
    rejected_proposals INTEGER DEFAULT 0,
    success_rate REAL DEFAULT 0.0,              -- successful / total
    avg_tid REAL DEFAULT 0.0,                   -- Average token efficiency
    total_tokens_spent INTEGER DEFAULT 0,
    last_active INTEGER                         -- Unix timestamp
);

-- Audit Log (every significant action)
CREATE TABLE aop_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,                 -- Unix timestamp
    actor TEXT NOT NULL,                        -- agent_uid or "user" or "system"
    action TEXT NOT NULL,                       -- "task_created", "diff_proposed", "diff_applied", etc.
    target_id TEXT,                             -- Task or mutation ID
    details TEXT                                -- JSON blob with context
);

CREATE INDEX idx_audit_timestamp ON aop_audit_log(timestamp);
CREATE INDEX idx_audit_actor ON aop_audit_log(actor);
```

---

## 11. Vector Index Engine

### Purpose

Enable agents to find relevant code by meaning, not just by filename or grep.

### AST-Aware Chunking

Do NOT split files by line count. Split by semantic structure:

```
Source file → Parse AST → Extract chunks:
  - Each function/method = 1 chunk
  - Each class = 1 chunk (without method bodies)
  - Each export block = 1 chunk
  - File-level imports + config = 1 chunk
```

Each chunk is stored with metadata:

```typescript
interface CodeChunk {
  id: string;
  filePath: string;
  chunkType: 'function' | 'class' | 'export' | 'imports' | 'config';
  name: string;           // Function/class name
  startLine: number;
  endLine: number;
  content: string;
  embedding: number[];    // Vector from embedding model
  dependencies: string[]; // What this chunk imports/calls
}
```

### Index Operations

```
index_project(projectPath)   → Scan all files, chunk, embed, store in LanceDB
query(naturalLanguage)       → Return top-K chunks by cosine similarity
update_chunk(filePath)       → Re-index only the changed file
```

### When to Re-index

- After every applied mutation (only the affected files)
- On user request ("re-scan project")
- NOT on every agent query (too expensive)

---

## 12. Data Integrity Rules

### Timestamps

| Context | Format | Example |
|---|---|---|
| Database columns | Unix timestamp (integer, seconds) | `1718234567` |
| Documentation files (.md) | `YYYY.MM.DD` | `2025.06.15` |
| UI display | Locale-aware via `Intl.DateTimeFormat` | "Jun 15, 2025" |
| Logs | ISO 8601 | `2025-06-15T14:22:47Z` |

**Rule**: Never store human-readable dates in the database. Never store Unix timestamps in documentation.

### Source-to-Doc Parity

A mutation is only `completed` (not just `applied`) when:

1. The diff has been applied to the target codebase
2. Any affected documentation (README, ADR, CHANGELOG) has been updated
3. Tier 1 validates that the documentation reflects the INTENT of the change

If no documentation needs updating, the agent must explicitly state: "No documentation impact."

### Checksums

Before any mutation is applied:
1. Calculate SHA-256 of all target files
2. Store as `checksum_before` in `aop_tasks`
3. After applying, calculate again and store as `checksum_after`
4. If `checksum_before` doesn't match the current file state → ABORT (someone else modified the file)

---

## 13. UI Dashboard

### Task Graph (React Flow)

- Each node = one task from `aop_tasks`
- Edges = parent_id relationships
- Node colors:
  - Gray: `pending`
  - Blue (pulsing): `executing`
  - Green: `completed`
  - Red: `failed`
  - Yellow: `paused` (needs human attention)
- Click node → expand to see child tasks, token usage, proposals
- Double-click → open Diff Reviewer for that task's mutations

### Token Burn Chart (Recharts)

- X axis: time
- Y axis (left): cumulative tokens spent
- Y axis (right): cumulative compliance score
- Alert condition: if `tokens_spent` increases by >1000 but `compliance_score` hasn't changed in 3 ticks → show warning "Low efficiency detected"

### Diff Reviewer

- Side-by-side view: original file vs proposed changes
- Header shows: agent persona, confidence score, intent description
- Action buttons: Approve, Reject (with reason), Request Revision

### Conflict Resolution UI

Shown when Tier 2 consensus fails:

- Display both proposals side by side
- Show semantic distance score
- Show each agent's reasoning
- User picks: Accept A, Accept B, Reject Both, Merge Manually

---

## 14. Error Handling & Recovery

### Error Categories

| Error | Severity | Auto-Recovery | Action |
|---|---|---|---|
| MCP server unreachable | HIGH | Retry 3x with backoff (1s, 5s, 15s) | If still fails → pause all tasks, alert user |
| LLM API timeout | MEDIUM | Retry 2x | If still fails → mark task as `failed`, log error |
| LLM returns invalid JSON | LOW | Retry 1x with stricter prompt | If still fails → mark task as `failed` |
| Shadow test CI fails | EXPECTED | No auto-retry | Reject mutation, log CI output |
| SQLite write fails | CRITICAL | No retry | Stop all operations, alert user |
| Vector DB out of sync | MEDIUM | Auto re-index affected files | If re-index fails → alert user |
| Diff can't apply (merge conflict) | MEDIUM | No retry | Reject mutation, suggest re-reading target file |
| Token budget exceeded | EXPECTED | No retry | Pause task, surface to user |
| Consensus deadlock (3 attempts) | MEDIUM | No retry | Escalate to user via Conflict Resolution UI |
| Checksum mismatch on apply | HIGH | Abort immediately | Alert user: "Target file was modified externally" |
| Tier 3 proposes change outside its domain | LOW | Auto-reject | Log as boundary violation, update agent metrics |

### Circuit Breakers

- **Token circuit breaker**: If a single domain uses >85% of its allocated budget, all tasks in that domain are `paused` and the user is notified
- **Failure circuit breaker**: If >3 consecutive Tier 3 calls fail for the same domain, stop spawning new agents for that domain
- **Depth limiter**: Maximum task tree depth = 3 (Tier 1 → Tier 2 → Tier 3). Any attempt to spawn a Tier 4 is auto-rejected with a log entry

### Recovery Playbook

When user sees a paused/failed task:

1. Click the task node in the graph
2. See: error message, tokens spent, what was accomplished so far
3. Options:
   - **Retry**: Reset status to `pending`, optionally increase budget
   - **Modify**: Change the objective or constraints, then retry
   - **Abort**: Mark as `failed` permanently, free up budget for other tasks
   - **Escalate**: Open task context in a new manual chat for human-guided resolution

---

## 15. Formulas & Thresholds

All formulas that were previously in images, now in plain text.

### Probabilistic Risk Assessment (PRA)

Used by Tier 1 before spawning sub-tasks:

```
Risk = P(failure) * Impact * (1 - TestCoverage)
```

Where:
- `P(failure)` = estimated probability of the change breaking something (0.0 to 1.0), based on file complexity and dependency count
- `Impact` = number of files/modules that depend on the changed code (normalized 0.0 to 1.0)
- `TestCoverage` = percentage of the affected code covered by tests (0.0 to 1.0)

**Thresholds**:
- Risk < 0.3 → Low risk. Tier 3 can proceed autonomously
- Risk 0.3 to 0.7 → Medium risk. Requires Tier 2 consensus check
- Risk > 0.7 → High risk. Requires Tier 1 final approval before any diff is applied

### Token Efficiency (TID)

```
TID = (compliance_gain / tokens_spent) * 100
```

**Thresholds**:
- TID > 2.0 → Excellent efficiency
- TID 0.5 to 2.0 → Acceptable
- TID < 0.5 → Poor. Tier 1 should consider: reducing scope, changing persona, or cutting budget

### Token Budget Allocation

```
domain_budget = (global_budget * 0.8) * (domain_weight / total_weight)
```

Where `domain_weight` is proportional to the number of files and complexity in that domain.

### Semantic Distance (for Consensus)

```
distance = 1 - cosine_similarity(embedding_A, embedding_B)
```

Where `embedding_A` and `embedding_B` are the intent embeddings of two competing diff proposals.

**Thresholds**:
- distance < 0.15 → Proposals are essentially the same. Pick the one with higher confidence
- distance 0.15 to 0.3 → Minor differences. Tier 2 can merge or pick best
- distance > 0.3 → Significant disagreement. Spawn arbitrator agent OR escalate to user

### Context Pause Threshold

```
pause_when: token_usage > token_budget * 0.85
```

At 85% budget consumption, the agent must stop and save state.

### Token Circuit Breaker

```
circuit_break_when: domain_token_usage > domain_budget * 0.85
```

At 85% domain budget, ALL tasks in that domain are paused.

---

## Execution Directive

Follow the phases in Section 3 **in order**. Each phase must compile, run, and pass its "done when" criteria before starting the next.

**Priority**: Working software over comprehensive features. A simple version that works beats a complex version that doesn't compile.

**Naming**: Use clear, descriptive names. `task_store.rs` not `sovereign_core.rs`. `diff_pipeline.rs` not `mutation_rail.rs`. Code reads better when names describe what something DOES, not what it's called in the architecture doc.

**Testing**: Each phase should include at minimum:
- Rust: integration test for the new Tauri commands
- React: smoke test that the new UI component renders
- Bridge: test that MCP tool calls return expected responses

**Commits**: One commit per phase, with message format: `feat(phase-N): <description>`
