# AOP UI Brainstorming Summary

**Quick Reference Guide**  
**Based on**: system.md + system_mcp.md  
**Output**: Complete UI/UX Architecture Plan

---

## 🎯 Core Design Decisions

### 1. **Information Architecture** - 6 Main Views

| View | Purpose | Key Features |
|------|---------|--------------|
| **Tasks** | Primary workspace | Task graph, agent hierarchy, execution monitoring |
| **Dashboard** | Metrics overview | Token burn, efficiency, agent pool, domain stats |
| **Context** | Semantic engine | Live queries, vector index status, code fragments |
| **Mutations** | Change approval | Diff review queue, side-by-side comparison, shadow tests |
| **Logs** | System events | Filterable stream, MCP activity, debug info |
| **System** | Health & settings | Component status, circuit breakers, configuration |

---

## 🎨 Visual Design System

### Color Semantics

**Task States**:
- ⚪ **Pending**: Gray
- 🔵 **Executing**: Blue (pulsing animation)
- 🟢 **Completed**: Green
- 🔴 **Failed**: Red
- 🟡 **Paused**: Yellow

**Risk Levels**:
- 🟢 **Low** (< 0.3): Can proceed autonomously
- 🟡 **Medium** (0.3-0.7): Requires consensus
- 🔴 **High** (> 0.7): Needs approval

**Agent Tiers**:
- 🟣 **Tier 1**: Purple (Orchestrator)
- 🔵 **Tier 2**: Blue (Domain Leaders)
- 🟢 **Tier 3**: Green (Specialists)

---

## 🔄 Key User Flows

### Creating a Task
```
Click "New Task" 
→ Fill objective 
→ Set budget (optional) 
→ Submit 
→ Tier 1 spawns 
→ Graph updates in real-time
```

### Reviewing a Mutation
```
Mutation appears in queue (badge notification)
→ Click mutation 
→ Review diff side-by-side 
→ Check shadow test results
→ Decision: Approve / Reject / Revise
→ Applied to codebase (if approved)
→ Vector index updates
```

### Context Exploration
```
Type search query 
→ Vector search executes 
→ Results ranked by relevance 
→ Click fragment 
→ See full code + metadata
→ Option to open in editor
```

---

## 📊 Real-time Features

### WebSocket Events (Immediate Updates)
- Task status changes
- Mutation proposals
- Token usage increments
- Context queries from agents
- MCP tool calls
- Circuit breaker state changes
- Security violations

### Polling (Periodic Updates)
- Dashboard metrics (5s)
- Index status (10s)
- Health checks (15s)

---

## 🧩 Custom Components

### Core Components
1. **TaskNode** (React Flow) - Visual task representation in graph
2. **DiffViewer** - Side-by-side code comparison with syntax highlighting
3. **ContextQueryVisualizer** - Live agent query timeline
4. **CircuitBreakerIndicator** - MCP server health status
5. **TokenBurnChart** - Cumulative token usage over time
6. **LogsStream** - Filterable event feed
7. **HealthMonitor** - System component status dashboard

### Component Features
- Keyboard shortcuts for power users
- Tooltips on hover for context
- Context menus on right-click
- Optimistic UI updates
- Error boundaries for resilience

---

## 🚀 Implementation Strategy

### Phase-by-Phase Approach

**Phase 1** (Weeks 1-2): Foundation
- Basic shell + task graph
- React Flow integration
- Mock data for development

**Phase 2** (Week 3): Backend Connection
- Tauri commands
- WebSocket integration
- Real agent spawning

**Phase 3** (Week 4): Mutation Pipeline
- Diff viewer
- Approval workflow
- Shadow test integration

**Phase 4** (Week 5): Context Visibility
- Vector search UI
- Live query monitoring
- Index status dashboard

**Phase 5** (Week 6): System Health
- Health monitors
- Circuit breakers
- Logs and diagnostics

**Phase 6** (Week 7): Polish
- Performance optimization
- Keyboard shortcuts
- Onboarding flow

---

## 💡 Key Innovation Points

### 1. **Transparency-First Design**
Unlike typical AI agents, AOP shows:
- Exactly what code the AI is reading
- Real-time token consumption
- Every proposed change before applying
- Complete audit trail of decisions

### 2. **Multi-Tier Visualization**
The task graph clearly shows:
- Agent hierarchy (Tier 1 → 2 → 3)
- Parent-child task relationships
- Token budget allocation by domain
- Execution state at every level

### 3. **Context Awareness**
Users can see:
- Which code fragments feed each agent
- Semantic search queries in real-time
- Vector index freshness
- AST-aware chunking results

### 4. **Zero-Trust Workflow**
Every mutation must:
- Pass shadow tests
- Be reviewed by user (or auto-approved for low-risk)
- Show confidence + risk scores
- Be traceable to originating agent

### 5. **System Observability**
Complete visibility into:
- MCP bridge health
- Circuit breaker states
- Rate limiting status
- Embedding model (local vs cloud)
- File watcher activity

---

## 🎯 Success Criteria

**The UI is successful when**:
1. Non-technical users can understand what agents are doing
2. Developers can debug agent behavior from the UI alone
3. All mutations are reviewable before applying
4. Token budget is always visible and controllable
5. System failures are immediately apparent
6. Recovery actions are one click away
7. Performance remains smooth with 100+ active tasks

---

## 📦 Tech Stack Summary

- **Desktop**: Tauri 2 (Rust)
- **Frontend**: React 19 + TypeScript
- **UI Components**: shadcn/ui (Tailwind v4)
- **Graph**: React Flow (@xyflow)
- **Charts**: Recharts
- **State**: Zustand + React Query
- **Styling**: Tailwind CSS v4 (OKLCH colors)
- **Icons**: Lucide React

---

## 🔑 Critical Implementation Notes

### Performance Optimizations
- **Virtualized lists** for logs (react-window)
- **Memo-ized** React Flow nodes
- **Debounced** search inputs
- **Batched** WebSocket events
- **Lazy-loaded** heavy components (Monaco editor for diffs)

### Accessibility
- Full keyboard navigation
- ARIA labels on all interactive elements
- Screen reader announcements for status changes
- High contrast mode support
- Reduced motion option

### Error Handling
- Error boundaries around each major component
- Toast notifications for user-facing errors
- Detailed error logs in Logs view
- Automatic retry with exponential backoff
- Clear recovery action buttons

---

**Next Steps**: Start with Phase 1 implementation (foundation + task graph)