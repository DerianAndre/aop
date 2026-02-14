# AOP UI/UX Architecture Plan

**Document**: AOP-UI-001  
**System References**: system.md, system_mcp.md  
**Design Lead**: Derian Castillo  
**Last Updated**: February 2026

---

## Table of Contents

1. [Design Philosophy](#1-design-philosophy)
2. [Information Architecture](#2-information-architecture)
3. [Core Views & Layouts](#3-core-views--layouts)
4. [Component Specifications](#4-component-specifications)
5. [Real-time Data Flows](#5-real-time-data-flows)
6. [Interaction Patterns](#6-interaction-patterns)
7. [Visual Design System](#7-visual-design-system)
8. [State Management Architecture](#8-state-management-architecture)
9. [Implementation Roadmap](#9-implementation-roadmap)

---

## 1. Design Philosophy

### Core Principles

**Transparency Over Automation**
- Users should ALWAYS know what the AI is doing, reading, and proposing
- Every mutation must be visible and reviewable before applying
- No "magic" - every decision should be traceable

**Progressive Disclosure**
- High-level overview by default
- Deep details available on demand
- 3-level information hierarchy: Glance → Scan → Deep Dive

**Real-time Awareness**
- Live status of agents, tasks, and system health
- Immediate feedback on token consumption
- Streaming updates during long operations

**Cognitive Load Management**
- Dense information, clean presentation
- Color-coded state machines (consistent across all views)
- Smart defaults with power-user escape hatches

---

## 2. Information Architecture

### Primary Navigation Structure

```
┌─────────────────────────────────────────────────────────────┐
│  AOP                                    [Project] [Settings] │
├─────────────────────────────────────────────────────────────┤
│  ┌─────┬─────────┬────────┬──────────┬─────────┬──────────┐ │
│  │ 🎯  │  📊     │  🧠    │  🔄      │  📝     │  ⚙️      │ │
│  │Tasks│Dashboard│Context │Mutations │Logs     │System    │ │
│  └─────┴─────────┴────────┴──────────┴─────────┴──────────┘ │
│                                                               │
│  [MAIN CONTENT AREA]                                         │
└─────────────────────────────────────────────────────────────┘
```

**Tab Hierarchy**:

1. **Tasks** (Primary View) - Task graph, agent execution, hierarchy
2. **Dashboard** - Metrics, charts, performance overview
3. **Context** - Vector index status, semantic engine, what agents "see"
4. **Mutations** - Diff pipeline, approval queue, change history
5. **Logs** - System events, MCP bridge activity, debug info
6. **System** - Settings, health monitors, circuit breakers

---

## 3. Core Views & Layouts

### 3.1 Tasks View (Primary)

**Layout**: Split panel with resizable divider

```
┌────────────────────────────────────────────────────────────┐
│  Tasks                                  [New Task] [Filter] │
├──────────────────────────┬─────────────────────────────────┤
│                          │                                  │
│   TASK GRAPH             │   TASK DETAILS                   │
│   (React Flow)           │   (Selected node info)           │
│                          │                                  │
│   ┌──[Root Task]──┐      │   Task: "Implement auth system" │
│   │   pending     │      │   ID: task_abc123                │
│   └───────┬───────┘      │   Status: executing              │
│           │              │   Agent: Tier 1 Orchestrator     │
│      ┌────┴────┐         │   Tokens: 1,240 / 5,000         │
│      │         │         │                                  │
│   ┌──▼──┐  ┌──▼──┐      │   Subtasks: 3 total              │
│   │T2-1 │  │T2-2 │      │   - T2-1: executing (40%)        │
│   │exec │  │pend │      │   - T2-2: pending                │
│   └──┬──┘  └─────┘      │   - T2-3: pending                │
│      │                  │                                  │
│   ┌──▼───┐              │   [View Agent Log]               │
│   │T3-1  │              │   [Adjust Budget]                │
│   │compl.│              │   [Pause/Resume]                 │
│   └──────┘              │                                  │
│                          │                                  │
│  Legend:                 │   Recent Activity:               │
│  ⚪ pending              │   14:32 - Tier 2 spawned T3-1    │
│  🔵 executing            │   14:30 - Started T2-1           │
│  🟢 completed            │   14:28 - Task created           │
│  🔴 failed               │                                  │
│  🟡 paused               │                                  │
└──────────────────────────┴─────────────────────────────────┘
```

**Key Features**:
- **Zoom/pan** on task graph with minimap
- **Auto-layout** using hierarchical algorithm (dagre)
- **Live updates** - nodes pulse when executing
- **Edge labels** - show dependency type (blocking, informational)
- **Node badges** - token usage, risk level, time elapsed
- **Context menu** - right-click for quick actions
- **Keyboard shortcuts** - arrow keys to navigate, Enter to expand

### 3.2 Dashboard View

**Layout**: Grid of metric cards + charts

```
┌────────────────────────────────────────────────────────────┐
│  Dashboard                              [Time Range: 24h]  │
├────────────────────────────────────────────────────────────┤
│  ┌────────────┐  ┌────────────┐  ┌────────────┐           │
│  │ 15 Tasks   │  │ 12.4K      │  │ 94.3%      │           │
│  │ Active     │  │ Tokens     │  │ System     │           │
│  │            │  │ Spent      │  │ Health     │           │
│  └────────────┘  └────────────┘  └────────────┘           │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Token Burn Over Time                                │  │
│  │  ┌─────────────────────────────────────────────────┐ │  │
│  │  │ ╱╱╱╱╱                        Cumulative tokens  │ │  │
│  │  │╱     ╲╱                                         │ │  │
│  │  │        ╲                                        │ │  │
│  │  └─────────────────────────────────────────────────┘ │  │
│  │  0:00   4:00   8:00   12:00   16:00   20:00   24:00 │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌──────────────┐  ┌──────────────────────────────────┐   │
│  │ Agent Pool   │  │  Efficiency by Domain            │   │
│  │              │  │  ┌──────────────────────────────┐│   │
│  │ Tier 1: 1/1  │  │  │ Auth:      TID 2.3  ████████ ││   │
│  │ Tier 2: 3/5  │  │  │ Database:  TID 1.1  ████     ││   │
│  │ Tier 3: 8/20 │  │  │ Frontend:  TID 0.8  ███      ││   │
│  │              │  │  └──────────────────────────────┘│   │
│  └──────────────┘  └──────────────────────────────────┘   │
└────────────────────────────────────────────────────────────┘
```

**Metrics Tracked**:
- Active tasks by tier
- Total tokens spent vs budget
- System health score (MCP bridge, vector engine, SQLite)
- Token burn rate (tokens/hour)
- TID (Token Impact Density) by domain
- Agent utilization %
- Average task completion time
- Mutation approval rate
- Circuit breaker status

### 3.3 Context View (Semantic Engine)

**Layout**: Three panels - Index status, Live queries, Fragment explorer

```
┌────────────────────────────────────────────────────────────┐
│  Context                                  [Reindex Project] │
├────────────────────────────────────────────────────────────┤
│  ┌────────────────────────────────────────────────────┐    │
│  │  Vector Index Status                               │    │
│  │  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 94% indexed   │    │
│  │                                                     │    │
│  │  📁 2,847 files  |  📦 15,234 chunks  |  🔄 12 stale│    │
│  │  Last indexed: 2 min ago                           │    │
│  │  Embedding model: BGE-M3 (local) + OpenAI fallback │    │
│  │  Index size: 1.2 GB                                │    │
│  └────────────────────────────────────────────────────┘    │
│                                                             │
│  ┌──────────────────────┐  ┌──────────────────────────┐   │
│  │ Live Agent Queries   │  │ Top Code Fragments       │   │
│  │                      │  │                          │   │
│  │ 🔵 T2-Auth           │  │ 1. src/auth/jwt.ts       │   │
│  │ "JWT token validity" │  │    verifyToken()         │   │
│  │ → 5 chunks loaded    │  │    Score: 0.94           │   │
│  │ ⏱ 87ms              │  │                          │   │
│  │                      │  │ 2. src/auth/middleware   │   │
│  │ 🔵 T3-Database       │  │    authMiddleware()      │   │
│  │ "Prisma migrations"  │  │    Score: 0.89           │   │
│  │ → 3 chunks loaded    │  │                          │   │
│  │ ⏱ 52ms              │  │ 3. src/config/auth.ts    │   │
│  │                      │  │    authConfig            │   │
│  │ 🟢 T3-Frontend       │  │    Score: 0.76           │   │
│  │ "Login component"    │  │                          │   │
│  │ → 8 chunks loaded    │  │ [Click to view full]     │   │
│  │ ⏱ 103ms             │  │                          │   │
│  └──────────────────────┘  └──────────────────────────┘   │
│                                                             │
│  File Watcher Activity:                                    │
│  • src/auth/jwt.ts modified → reindexing...               │
│  • src/types/user.ts created → indexing...                │
└────────────────────────────────────────────────────────────┘
```

**Key Features**:
- **Real-time query visualization** - See what agents are asking for
- **Fragment preview** - Click to see full code chunk
- **Embedding model indicator** - Show if local or cloud was used
- **Staleness warnings** - Highlight chunks that need reindexing
- **Manual search** - User can query the index directly
- **Watcher log** - Live feed of filesystem events

### 3.4 Mutations View (Diff Pipeline)

**Layout**: Queue + Detailed reviewer

```
┌────────────────────────────────────────────────────────────┐
│  Mutations                         [Approve All] [Settings] │
├──────────────────────┬─────────────────────────────────────┤
│  APPROVAL QUEUE      │  DIFF REVIEWER                      │
│                      │                                      │
│  ⏳ Pending (3)      │  Mutation #M-1547                    │
│                      │  Task: T2-Auth-1                     │
│  🟡 M-1547           │  Agent: Tier 3 Backend Specialist    │
│     Add JWT refresh  │  Risk: 🟢 Low (0.23)                │
│     • 2 files        │  Confidence: 87%                     │
│     • +45 -12 lines  │                                      │
│                      │  ┌──────────────────────────────────┐│
│  🟡 M-1548           │  │ Intent:                          ││
│     Update auth mid. │  │ "Add token refresh endpoint to   ││
│     • 1 file         │  │  prevent session expiration."    ││
│     • +23 -5 lines   │  └──────────────────────────────────┘│
│                      │                                      │
│  🟡 M-1549           │  📁 src/auth/jwt.ts                  │
│     Type definitions │  ┌──────────────────────────────────┐│
│     • 1 file         │  │@@ -45,6 +45,23 @@              ││
│     • +8 -0 lines    │  │ export function verifyToken() { ││
│                      │  │   // existing code...            ││
│  ─────────────────── │  │ }                                ││
│                      │  │                                  ││
│  ✅ Applied (12)     │  │+export function refreshToken(   ││
│  └─ M-1546           │  │+  oldToken: string              ││
│  └─ M-1545           │  │+): TokenResponse {              ││
│  └─ M-1544           │  │+  const decoded = verify(old..  ││
│                      │  │+  return generateToken(decoded) ││
│  ❌ Rejected (2)     │  │+}                                ││
│  └─ M-1543           │  └──────────────────────────────────┘│
│  └─ M-1542           │                                      │
│                      │  Shadow Test Results:                │
│  📊 Stats            │  ✅ Unit tests: 24/24 passed         │
│  Approval rate: 85%  │  ✅ Type check: passed               │
│  Avg review: 2.3m    │  ⚠️  E2E tests: 2 skipped           │
│                      │                                      │
│                      │  [✅ Approve] [❌ Reject] [✏️ Revise]│
└──────────────────────┴─────────────────────────────────────┘
```

**Approval Workflow States**:
- 🟡 **Pending**: Awaiting review
- 🔵 **Testing**: Shadow CI running
- 🟢 **Approved**: Ready to apply
- 🔴 **Rejected**: User or tests rejected
- ⚫ **Applied**: Successfully written to target

### 3.5 Logs View

**Layout**: Filterable event stream

```
┌────────────────────────────────────────────────────────────┐
│  Logs                     [Filter] [Export] [Clear]        │
├────────────────────────────────────────────────────────────┤
│  [●] System  [●] MCP  [●] Agents  [●] Vector  [ ] Debug    │
│                                                             │
│  14:45:23 [MCP] ✅ Filesystem server: read_file            │
│           src/auth/jwt.ts (245 bytes)                      │
│                                                             │
│  14:45:22 [Vector] 🔍 Query: "JWT token validation"        │
│           → 5 chunks (BGE-M3 local, 87ms)                  │
│                                                             │
│  14:45:20 [Agent] 🤖 T3-Backend-1 started                  │
│           Task: T2-Auth-1-sub-1                            │
│           Budget: 2000 tokens                              │
│                                                             │
│  14:45:18 [MCP] ⚠️ Rate limit: 98/120 calls/min            │
│                                                             │
│  14:45:15 [System] 🔄 Sidecar heartbeat OK                 │
│           Uptime: 2h 34m                                   │
│                                                             │
│  14:45:10 [Agent] ✅ T3-Frontend-2 completed               │
│           Tokens used: 1,847 / 3,000                       │
│                                                             │
│  14:45:05 [MCP] ❌ SECURITY_VIOLATION blocked              │
│           Agent attempted: ../../etc/passwd                │
│           Task: T2-System-1 → ABORTED                      │
│                                                             │
│  [Load more...] Showing 50 of 2,847 events                │
└────────────────────────────────────────────────────────────┘
```

**Log Categories**:
- **System**: App lifecycle, crashes, health checks
- **MCP**: Bridge activity, tool calls, security blocks
- **Agents**: Task lifecycle, token usage, completions
- **Vector**: Index operations, queries, reindexing
- **Debug**: Verbose internal state (off by default)

### 3.6 System View

**Layout**: Health monitors + Settings

```
┌────────────────────────────────────────────────────────────┐
│  System                                       [Diagnostics] │
├────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Component Health                                    │  │
│  │                                                       │  │
│  │  🟢 MCP Bridge                              OK  87ms │  │
│  │     Sidecar uptime: 2h 34m                          │  │
│  │     Active servers: 3 (filesystem, git, database)   │  │
│  │     Circuit breakers: All closed                    │  │
│  │                                                       │  │
│  │  🟢 Vector Engine                           OK  52ms │  │
│  │     LanceDB size: 1.2 GB                            │  │
│  │     Chunks indexed: 15,234                          │  │
│  │     Pending embeddings: 0                           │  │
│  │                                                       │  │
│  │  🟢 SQLite Database                         OK   3ms │  │
│  │     Size: 34.2 MB                                   │  │
│  │     Tasks: 247  |  Mutations: 1,829                │  │
│  │     Last backup: 14 min ago                         │  │
│  │                                                       │  │
│  │  🟡 API Quota                            WARN  78%  │  │
│  │     OpenAI tokens: 78K / 100K (resets in 6h)       │  │
│  │     Anthropic tokens: 45K / 200K                   │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  Settings:                                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Global Token Budget: [50000] tokens/day            │  │
│  │  Max Task Depth:      [3] tiers                     │  │
│  │  Auto-approve low-risk: [✓] (Risk < 0.3)            │  │
│  │  Embedding model:     [● BGE-M3 local + cloud]      │  │
│  │  Theme:               [● Dark  ○ Light  ○ Auto]     │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

---

## 4. Component Specifications

### 4.1 TaskNode Component (React Flow)

**Props**:
```typescript
interface TaskNodeProps {
  id: string;
  data: {
    tier: 1 | 2 | 3;
    objective: string;
    status: TaskStatus;
    tokens_spent: number;
    tokens_budget: number;
    risk_score: number;
    agent_persona: string;
    created_at: number;
  };
}
```

**Visual States**:
```css
.task-node {
  /* Base */
  border-radius: 8px;
  border: 2px solid;
  padding: 12px;
  min-width: 200px;
  
  /* Status-based colors */
  &[data-status="pending"]   { border-color: hsl(var(--muted)); }
  &[data-status="executing"] { 
    border-color: hsl(var(--primary));
    animation: pulse 2s infinite;
  }
  &[data-status="completed"] { border-color: hsl(142 76% 36%); }
  &[data-status="failed"]    { border-color: hsl(var(--destructive)); }
  &[data-status="paused"]    { border-color: hsl(45 93% 47%); }
}
```

**Interaction**:
- **Click**: Select node → show details in right panel
- **Double-click**: Expand/collapse subtasks
- **Right-click**: Context menu (Pause, Resume, Abort, Retry)
- **Hover**: Show tooltip with full objective + token usage

### 4.2 DiffViewer Component

**Props**:
```typescript
interface DiffViewerProps {
  mutation: {
    id: string;
    file_path: string;
    original_content: string;
    proposed_content: string;
    agent_intent: string;
    confidence_score: number;
    risk_score: number;
  };
  onApprove: () => void;
  onReject: (reason: string) => void;
  onRevise: (feedback: string) => void;
}
```

**Layout**:
- Split view: 50/50 original vs proposed
- Line-level diff highlighting
- Syntax highlighting by language
- Collapsed unchanged sections (click to expand)
- Line numbers aligned across both sides
- Intent description header
- Risk badge (color-coded)

**Keyboard shortcuts**:
- `Cmd/Ctrl + Enter`: Approve
- `Cmd/Ctrl + R`: Reject
- `Tab`: Next mutation in queue
- `Shift + Tab`: Previous mutation

### 4.3 ContextQueryVisualizer Component

**Purpose**: Show live semantic queries from agents

```typescript
interface ContextQuery {
  agent_id: string;
  agent_tier: number;
  query: string;
  results: CodeChunk[];
  latency_ms: number;
  embedding_source: 'local' | 'cloud';
  timestamp: number;
}
```

**Visualization**:
- Timeline view (queries over last 5 minutes)
- Each query shows: agent name, query text, # chunks returned, latency
- Click query → expand to see actual code fragments
- Color-code by tier (Tier 1: purple, Tier 2: blue, Tier 3: green)
- Latency sparkline (historical query times)

### 4.4 CircuitBreakerIndicator Component

**States**:
```typescript
type CircuitState = 'closed' | 'open' | 'half-open';

interface CircuitBreakerStatus {
  server_id: string;
  state: CircuitState;
  failure_count: number;
  last_failure_at?: number;
  opens_at?: number; // When it will attempt half-open
}
```

**Visual**:
- Closed: 🟢 Green circle
- Open: 🔴 Red circle + countdown timer
- Half-open: 🟡 Yellow circle + "Testing..."
- Tooltip shows failure history

---

## 5. Real-time Data Flows

### 5.1 WebSocket Event Stream

**Backend → Frontend Events**:

```typescript
type AopEvent = 
  | { type: 'task_created', task_id: string, parent_id: string }
  | { type: 'task_status_changed', task_id: string, new_status: TaskStatus }
  | { type: 'mutation_proposed', mutation_id: string, task_id: string }
  | { type: 'mutation_approved', mutation_id: string }
  | { type: 'mutation_applied', mutation_id: string }
  | { type: 'token_usage', task_id: string, tokens_spent: number }
  | { type: 'context_query', agent_id: string, query: ContextQuery }
  | { type: 'index_updated', affected_files: string[] }
  | { type: 'mcp_event', server_id: string, event_data: any }
  | { type: 'circuit_breaker_state', server_id: string, state: CircuitState }
  | { type: 'error', error_code: string, message: string };
```

**Event Handler Architecture**:

```typescript
// Zustand store with event subscription
const useAopStore = create<AopState>((set, get) => ({
  tasks: new Map(),
  mutations: new Map(),
  
  // Event handlers
  handleEvent: (event: AopEvent) => {
    switch (event.type) {
      case 'task_status_changed':
        set(state => ({
          tasks: new Map(state.tasks).set(event.task_id, {
            ...state.tasks.get(event.task_id),
            status: event.new_status
          })
        }));
        break;
      // ... other handlers
    }
  }
}));

// WebSocket connection
useEffect(() => {
  const ws = new WebSocket('ws://localhost:1420/events');
  ws.onmessage = (msg) => {
    const event = JSON.parse(msg.data);
    useAopStore.getState().handleEvent(event);
  };
  return () => ws.close();
}, []);
```

### 5.2 Tauri Command Flow

**Frontend calls → Rust core → Response**:

```typescript
// Example: Starting a new task
import { invoke } from '@tauri-apps/api/core';

async function createTask(objective: string, parent_id?: string) {
  const task = await invoke<Task>('create_task', {
    objective,
    parent_id,
    tier: parent_id ? 2 : 1,
    token_budget: 5000
  });
  
  // Optimistic update
  useAopStore.getState().addTask(task);
  
  return task;
}
```

**Critical Commands**:
- `create_task(objective, parent_id, tier, budget)`
- `approve_mutation(mutation_id)`
- `reject_mutation(mutation_id, reason)`
- `pause_task(task_id)`
- `query_context(query, top_k)`
- `get_index_status()`
- `call_mcp_tool(request)`

### 5.3 Polling vs Streaming

**Use Polling For**:
- Dashboard metrics (every 5s)
- Index status (every 10s)
- Health checks (every 15s)

**Use WebSocket Streaming For**:
- Task state changes (immediate)
- Mutation proposals (immediate)
- Token usage updates (immediate)
- Log events (immediate)
- Context queries (immediate)

---

## 6. Interaction Patterns

### 6.1 Task Creation Flow

```
User Action: Click "New Task" button
    ↓
Modal opens with form:
  - Objective (required, textarea)
  - Parent task (optional, dropdown)
  - Token budget (optional, default based on tier)
  - Advanced: Risk threshold, auto-approve settings
    ↓
User fills objective: "Implement user authentication"
    ↓
Click "Create Task"
    ↓
Frontend: invoke('create_task', {...})
    ↓
Rust: Insert into aop_tasks, spawn Tier 1 agent
    ↓
Rust: Emit 'task_created' event via WebSocket
    ↓
Frontend: Update task graph, select new node
    ↓
Task card appears in graph with "executing" pulse
    ↓
Agent starts working, emits context queries
    ↓
Frontend: Show live queries in Context view
```

### 6.2 Mutation Review Flow

```
Agent proposes diff
    ↓
Backend: Insert into aop_mutations, set status = 'pending'
    ↓
Backend: Run shadow tests in isolated environment
    ↓
WebSocket event: 'mutation_proposed' → Frontend
    ↓
UI: Badge appears on "Mutations" tab (red dot)
    ↓
User clicks tab, sees mutation in queue
    ↓
User selects mutation M-1547
    ↓
DiffViewer loads:
  - Shows intent, confidence, risk
  - Renders side-by-side diff
  - Shows test results (streaming if still running)
    ↓
User reviews code changes
    ↓
Decision point:
  ├─ Approve → invoke('approve_mutation')
  │    ↓
  │   Backend: Apply diff to target codebase
  │    ↓
  │   Backend: Update aop_mutations.status = 'applied'
  │    ↓
  │   Backend: Re-index affected files
  │    ↓
  │   WebSocket: 'mutation_applied'
  │    ↓
  │   UI: Move mutation to "Applied" section
  │
  ├─ Reject → Modal for reason
  │    ↓
  │   invoke('reject_mutation', reason)
  │    ↓
  │   Backend: Update status = 'rejected'
  │    ↓
  │   Agent learns from rejection
  │
  └─ Revise → Modal for feedback
       ↓
      invoke('request_revision', feedback)
       ↓
      Backend: Create new sub-task with revision request
       ↓
      Agent generates new proposal
```

### 6.3 Context Search Flow

```
User in Context view
    ↓
Types in search bar: "authentication middleware"
    ↓
Frontend: invoke('query_context', {
  query: "authentication middleware",
  top_k: 10
})
    ↓
Rust: Embed query using BGE-M3
    ↓
Rust: LanceDB similarity search
    ↓
Rust: Re-rank using S(c, q) formula
    ↓
Returns: Vec<ContextChunk>
    ↓
UI: Display results in fragment explorer
    ↓
User clicks chunk #3
    ↓
Modal opens with:
  - Full code content (syntax highlighted)
  - File path (clickable to open in editor)
  - Relevance score
  - Parent symbol
  - Imports
  - "Used by agents" badge if any active query matched this
```

### 6.4 Error Recovery Flow

```
Task fails (e.g., MCP timeout)
    ↓
Backend: Update task status = 'failed'
    ↓
Backend: Log error with context
    ↓
WebSocket: 'task_status_changed' + 'error' events
    ↓
UI: Task node turns red
    ↓
User clicks failed task
    ↓
Right panel shows:
  - Error message
  - What was accomplished before failure
  - Tokens already spent
  - Recovery options:
    [Retry] [Modify Objective] [Abort] [Escalate to Manual]
    ↓
User clicks "Retry"
    ↓
invoke('retry_task', task_id)
    ↓
Backend: Reset status to 'pending'
    ↓
Backend: Re-spawn agent with same context
    ↓
WebSocket: 'task_status_changed'
    ↓
UI: Node returns to executing state
```

---

## 7. Visual Design System

### 7.1 Color Semantics

**Task/Mutation States**:
```css
:root {
  /* Task states */
  --status-pending: oklch(0.708 0 0);     /* Gray */
  --status-executing: oklch(0.6 0.24 252); /* Blue */
  --status-completed: oklch(0.65 0.22 145); /* Green */
  --status-failed: oklch(0.577 0.245 27);   /* Red */
  --status-paused: oklch(0.78 0.18 75);     /* Yellow */
  
  /* Risk levels */
  --risk-low: oklch(0.65 0.22 145);        /* Green */
  --risk-medium: oklch(0.78 0.18 75);      /* Yellow */
  --risk-high: oklch(0.577 0.245 27);      /* Red */
  
  /* Agent tiers */
  --tier-1: oklch(0.65 0.24 295);          /* Purple */
  --tier-2: oklch(0.6 0.24 252);           /* Blue */
  --tier-3: oklch(0.65 0.22 145);          /* Green */
}
```

**Health Indicators**:
- 🟢 Green: Operational, healthy
- 🟡 Yellow: Warning, degraded
- 🔴 Red: Critical, failing
- ⚫ Gray: Offline, disabled

### 7.2 Typography Scale

```css
/* Tailwind v4 - using font-size utilities */
.text-display {
  font-size: 3rem;      /* 48px - Dashboard titles */
  line-height: 1.2;
}

.text-heading {
  font-size: 1.5rem;    /* 24px - Section headers */
  line-height: 1.3;
}

.text-body {
  font-size: 0.875rem;  /* 14px - Default UI text */
  line-height: 1.5;
}

.text-caption {
  font-size: 0.75rem;   /* 12px - Metadata, timestamps */
  line-height: 1.4;
  color: oklch(var(--muted-foreground));
}

.text-code {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 0.8125rem; /* 13px */
  line-height: 1.6;
}
```

### 7.3 Spacing System

**Base unit**: `0.25rem` (4px)

```
spacing-1  = 4px   (tight elements)
spacing-2  = 8px   (card padding)
spacing-3  = 12px  (component margins)
spacing-4  = 16px  (section padding)
spacing-6  = 24px  (panel separation)
spacing-8  = 32px  (page margins)
spacing-12 = 48px  (large gaps)
```

### 7.4 Animation Library

```css
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

@keyframes slideIn {
  from { 
    transform: translateX(100%);
    opacity: 0;
  }
  to { 
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* Usage */
.task-executing {
  animation: pulse 2s ease-in-out infinite;
}

.mutation-new {
  animation: slideIn 0.3s ease-out;
}

.panel-load {
  animation: fadeIn 0.2s ease-out;
}
```

---

## 8. State Management Architecture

### 8.1 Zustand Store Structure

```typescript
interface AopState {
  // Core data
  tasks: Map<string, Task>;
  mutations: Map<string, Mutation>;
  contextQueries: ContextQuery[];
  
  // UI state
  selectedTaskId: string | null;
  selectedMutationId: string | null;
  activeTab: 'tasks' | 'dashboard' | 'context' | 'mutations' | 'logs' | 'system';
  
  // Filters
  taskFilter: {
    status?: TaskStatus[];
    tier?: (1 | 2 | 3)[];
    searchQuery?: string;
  };
  
  // System state
  indexStatus: IndexStatus;
  sidecarHealth: SidecarHealth;
  circuitBreakers: Map<string, CircuitBreakerStatus>;
  
  // Actions
  addTask: (task: Task) => void;
  updateTask: (taskId: string, updates: Partial<Task>) => void;
  addMutation: (mutation: Mutation) => void;
  selectTask: (taskId: string) => void;
  selectMutation: (mutationId: string) => void;
  
  // Event handler
  handleEvent: (event: AopEvent) => void;
}

const useAopStore = create<AopState>()(
  devtools(
    persist(
      (set, get) => ({
        // Initial state
        tasks: new Map(),
        mutations: new Map(),
        contextQueries: [],
        selectedTaskId: null,
        selectedMutationId: null,
        activeTab: 'tasks',
        taskFilter: {},
        indexStatus: defaultIndexStatus,
        sidecarHealth: defaultHealth,
        circuitBreakers: new Map(),
        
        // Actions implementation
        addTask: (task) => set((state) => ({
          tasks: new Map(state.tasks).set(task.id, task)
        })),
        
        updateTask: (taskId, updates) => set((state) => {
          const task = state.tasks.get(taskId);
          if (!task) return state;
          return {
            tasks: new Map(state.tasks).set(taskId, { ...task, ...updates })
          };
        }),
        
        // ... other actions
        
        handleEvent: (event) => {
          // Central event dispatch
          // Implementation shown in section 5.1
        }
      }),
      {
        name: 'aop-storage',
        partialize: (state) => ({
          // Only persist user preferences
          activeTab: state.activeTab,
          taskFilter: state.taskFilter,
        })
      }
    )
  )
);
```

### 8.2 React Query Integration

**For polling data**:

```typescript
// Dashboard metrics
const useMetrics = () => {
  return useQuery({
    queryKey: ['metrics'],
    queryFn: async () => invoke<Metrics>('get_metrics'),
    refetchInterval: 5000, // Poll every 5 seconds
    staleTime: 4000,
  });
};

// Index status
const useIndexStatus = () => {
  return useQuery({
    queryKey: ['index-status'],
    queryFn: async () => invoke<IndexStatus>('get_index_status'),
    refetchInterval: 10000,
  });
};
```

**For mutations**:

```typescript
const useMutateTask = () => {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async ({ objective }: { objective: string }) => 
      invoke<Task>('create_task', { objective }),
    
    onSuccess: (newTask) => {
      // Optimistic update
      queryClient.setQueryData(['tasks'], (old: Task[]) => 
        [...old, newTask]
      );
    },
  });
};
```

---

## 9. Implementation Roadmap

### Phase 1: Foundation (Week 1-2)

**Goal**: Basic shell with task graph

- [ ] Set up Tauri + React project structure
- [ ] Install shadcn/ui components (button, card, badge, tabs)
- [ ] Create main navigation layout
- [ ] Implement TaskNode component with React Flow
- [ ] Connect to Zustand store for task state
- [ ] Add mock WebSocket for development
- [ ] Basic task graph rendering (no real backend yet)

**Done when**:
- Can create mock tasks and see them in graph
- Can click nodes and see details panel
- Navigation between tabs works
- Dark mode toggles properly

### Phase 2: Real-time Backend Connection (Week 3)

**Goal**: Connect UI to Rust backend

- [ ] Implement Tauri commands for task CRUD
- [ ] Set up WebSocket event stream from Rust
- [ ] Wire up Zustand event handlers
- [ ] Implement real task creation flow
- [ ] Add token usage display
- [ ] Create basic dashboard with metrics

**Done when**:
- Creating a task in UI spawns real Tier 1 agent
- Task status changes reflect in graph immediately
- Token usage updates in real-time
- Dashboard shows live metrics

### Phase 3: Mutation Pipeline UI (Week 4)

**Goal**: Diff review and approval workflow

- [ ] Create DiffViewer component
- [ ] Implement syntax highlighting
- [ ] Build mutation approval queue
- [ ] Add approve/reject actions
- [ ] Integrate shadow test results display
- [ ] Create conflict resolution modal

**Done when**:
- Agent proposals appear in queue automatically
- Can review diffs side-by-side
- Approve/reject flows work end-to-end
- Test results show inline

### Phase 4: Context Visibility (Week 5)

**Goal**: Show semantic engine activity

- [ ] Create Context view layout
- [ ] Implement live query visualizer
- [ ] Build fragment explorer with search
- [ ] Add index status dashboard
- [ ] Show file watcher activity feed
- [ ] Create manual re-index trigger

**Done when**:
- Can see what agents are querying in real-time
- Can search the vector index manually
- Index status updates reflect reality
- Re-indexing triggers and shows progress

### Phase 5: System Health & Monitoring (Week 6)

**Goal**: Observability and diagnostics

- [ ] Build system health dashboard
- [ ] Implement circuit breaker indicators
- [ ] Create MCP bridge status monitor
- [ ] Add settings panel
- [ ] Build logs view with filtering
- [ ] Create error recovery UI

**Done when**:
- All system components show health status
- Circuit breaker states are visible
- Can filter and search logs
- Error recovery options are actionable

### Phase 6: Polish & Performance (Week 7)

**Goal**: Production-ready UX

- [ ] Add keyboard shortcuts
- [ ] Implement virtualized lists for large datasets
- [ ] Optimize React Flow performance (>100 nodes)
- [ ] Add loading skeletons
- [ ] Create onboarding tutorial
- [ ] Add export/import functionality
- [ ] Performance profiling and optimization

**Done when**:
- UI feels snappy with 100+ tasks
- No janky animations or lag
- First-time user can understand the interface
- Can export task history / mutation log

---

## Appendix A: Component Inventory

**shadcn/ui components to install**:

```bash
npx shadcn@latest add button
npx shadcn@latest add card
npx shadcn@latest add badge
npx shadcn@latest add tabs
npx shadcn@latest add dialog
npx shadcn@latest add scroll-area
npx shadcn@latest add select
npx shadcn@latest add input
npx shadcn@latest add textarea
npx shadcn@latest add tooltip
npx shadcn@latest add dropdown-menu
npx shadcn@latest add separator
npx shadcn@latest add progress
npx shadcn@latest add alert
npx shadcn@latest add toast
npx shadcn@latest add switch
```

**Custom components to build**:
- TaskNode (React Flow)
- DiffViewer
- ContextQueryVisualizer
- CircuitBreakerIndicator
- TokenBurnChart (Recharts)
- LogsStream
- HealthMonitor
- TaskDetailsPanel
- MutationQueue
- FragmentExplorer

---

## Appendix B: Accessibility Checklist

- [ ] All interactive elements keyboard navigable
- [ ] Focus visible on all focusable elements
- [ ] ARIA labels on icon-only buttons
- [ ] Color not the only indicator (use icons + text)
- [ ] Sufficient contrast ratios (WCAG AA minimum)
- [ ] Screen reader tested with NVDA/JAWS
- [ ] Reduced motion respects `prefers-reduced-motion`
- [ ] All modals trap focus
- [ ] Escape key closes modals/dropdowns
- [ ] Status changes announced to screen readers

---

**End of Document**