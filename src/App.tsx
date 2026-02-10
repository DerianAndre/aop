import { useEffect, useMemo, useState } from 'react'
import type { FormEvent } from 'react'

import {
  createTask,
  getDefaultTargetProject,
  getTasks,
  indexTargetProject,
  listTargetDir,
  orchestrateObjective,
  queryCodebase,
  readTargetFile,
  searchTargetFiles,
} from '@/hooks/useTauri'
import { cn } from '@/lib/utils'
import type {
  ContextChunk,
  CreateTaskInput,
  DirectoryEntry,
  DirectoryListing,
  IndexProjectResult,
  OrchestrationResult,
  SearchResult,
  TargetFileContent,
  TaskAssignment,
  TaskRecord,
  TaskStatus,
} from '@/types'

const STATUS_CLASS: Record<TaskStatus, string> = {
  pending: 'status-pending',
  executing: 'status-executing',
  completed: 'status-completed',
  failed: 'status-failed',
  paused: 'status-paused',
}

const DEFAULT_FORM: CreateTaskInput = {
  tier: 1,
  domain: 'platform',
  objective: '',
  tokenBudget: 3000,
}

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000))
}

function parseCommandArgs(rawArgs: string): string[] | undefined {
  const values = rawArgs
    .split(' ')
    .map((value) => value.trim())
    .filter(Boolean)

  return values.length > 0 ? values : undefined
}

function entryIcon(entry: DirectoryEntry): string {
  return entry.isDir ? 'DIR' : 'FILE'
}

function assignmentRiskLabel(assignment: TaskAssignment): string {
  if (assignment.riskFactor > 0.7) {
    return 'High'
  }
  if (assignment.riskFactor >= 0.3) {
    return 'Medium'
  }
  return 'Low'
}

function App() {
  const [tasks, setTasks] = useState<TaskRecord[]>([])
  const [formState, setFormState] = useState<CreateTaskInput>(DEFAULT_FORM)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isLoadingTasks, setIsLoadingTasks] = useState(true)
  const [taskFeedback, setTaskFeedback] = useState<string | null>(null)
  const [orchestratorObjective, setOrchestratorObjective] = useState('')
  const [orchestratorBudget, setOrchestratorBudget] = useState(12000)
  const [orchestratorRiskTolerance, setOrchestratorRiskTolerance] = useState(0.6)
  const [isOrchestrating, setIsOrchestrating] = useState(false)
  const [orchestrationResult, setOrchestrationResult] = useState<OrchestrationResult | null>(null)
  const [orchestratorFeedback, setOrchestratorFeedback] = useState<string | null>(null)

  const [targetProject, setTargetProject] = useState('')
  const [mcpCommand, setMcpCommand] = useState('')
  const [mcpArgs, setMcpArgs] = useState('')
  const [directory, setDirectory] = useState<DirectoryListing | null>(null)
  const [selectedFile, setSelectedFile] = useState<TargetFileContent | null>(null)
  const [searchPattern, setSearchPattern] = useState('')
  const [searchResult, setSearchResult] = useState<SearchResult | null>(null)
  const [isBrowsing, setIsBrowsing] = useState(false)
  const [browserFeedback, setBrowserFeedback] = useState<string | null>(null)
  const [semanticQuery, setSemanticQuery] = useState('')
  const [semanticResults, setSemanticResults] = useState<ContextChunk[]>([])
  const [indexResult, setIndexResult] = useState<IndexProjectResult | null>(null)
  const [isIndexing, setIsIndexing] = useState(false)
  const [isSemanticSearching, setIsSemanticSearching] = useState(false)

  const taskCountLabel = useMemo(() => {
    const count = tasks.length
    return `${count} task${count === 1 ? '' : 's'}`
  }, [tasks.length])

  useEffect(() => {
    void loadTasks()
    void loadDefaultTargetProject()
  }, [])

  async function loadDefaultTargetProject() {
    try {
      const projectPath = await getDefaultTargetProject()
      setTargetProject(projectPath)
    } catch {
      // Keep target project field user-driven when current directory isn't available.
    }
  }

  async function loadTasks() {
    setIsLoadingTasks(true)
    setTaskFeedback(null)

    try {
      const records = await getTasks()
      setTasks(records)
    } catch (error) {
      setTaskFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoadingTasks(false)
    }
  }

  async function handleTaskSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setTaskFeedback(null)

    if (!formState.objective.trim()) {
      setTaskFeedback('Objective is required.')
      return
    }

    setIsSubmitting(true)

    try {
      const createdTask = await createTask({
        ...formState,
        domain: formState.domain.trim(),
        objective: formState.objective.trim(),
      })

      setTasks((current) => [createdTask, ...current])
      setFormState((previous) => ({ ...previous, objective: '' }))
    } catch (error) {
      setTaskFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsSubmitting(false)
    }
  }

  async function handleOrchestrateSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setOrchestratorFeedback(null)

    const objective = orchestratorObjective.trim()
    const target = targetProject.trim()
    if (!target) {
      setOrchestratorFeedback('Target project path is required.')
      return
    }
    if (!objective) {
      setOrchestratorFeedback('Objective is required.')
      return
    }
    if (!Number.isFinite(orchestratorBudget) || orchestratorBudget < 100) {
      setOrchestratorFeedback('Global token budget must be at least 100.')
      return
    }
    if (!Number.isFinite(orchestratorRiskTolerance) || orchestratorRiskTolerance < 0 || orchestratorRiskTolerance > 1) {
      setOrchestratorFeedback('Max risk tolerance must be between 0.0 and 1.0.')
      return
    }

    setIsOrchestrating(true)

    try {
      const result = await orchestrateObjective({
        objective,
        targetProject: target,
        globalTokenBudget: Math.floor(orchestratorBudget),
        maxRiskTolerance: Number(orchestratorRiskTolerance.toFixed(2)),
      })

      setOrchestrationResult(result)
      setOrchestratorObjective('')
      await loadTasks()
    } catch (error) {
      setOrchestratorFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsOrchestrating(false)
    }
  }

  function buildMcpConfig() {
    const command = mcpCommand.trim()
    if (!command) {
      return {}
    }

    return {
      mcpCommand: command,
      mcpArgs: parseCommandArgs(mcpArgs),
    }
  }

  async function browseDirectory(dirPath = '.'): Promise<void> {
    const target = targetProject.trim()
    if (!target) {
      setBrowserFeedback('Target project path is required.')
      return
    }

    setIsBrowsing(true)
    setBrowserFeedback(null)

    try {
      const listing = await listTargetDir({
        targetProject: target,
        dirPath,
        ...buildMcpConfig(),
      })

      setDirectory(listing)
      setSearchResult(null)
      setSelectedFile(null)
      if (listing.warnings.length > 0) {
        setBrowserFeedback(listing.warnings.join('\n'))
      }
    } catch (error) {
      setBrowserFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsBrowsing(false)
    }
  }

  async function openFile(filePath: string): Promise<void> {
    const target = targetProject.trim()
    if (!target) {
      setBrowserFeedback('Target project path is required.')
      return
    }

    setIsBrowsing(true)
    setBrowserFeedback(null)

    try {
      const file = await readTargetFile({
        targetProject: target,
        filePath,
        ...buildMcpConfig(),
      })

      setSelectedFile(file)
      if (file.warnings.length > 0) {
        setBrowserFeedback(file.warnings.join('\n'))
      }
    } catch (error) {
      setBrowserFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsBrowsing(false)
    }
  }

  async function handleSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const target = targetProject.trim()
    const pattern = searchPattern.trim()

    if (!target) {
      setBrowserFeedback('Target project path is required.')
      return
    }
    if (!pattern) {
      setBrowserFeedback('Search pattern is required.')
      return
    }

    setIsBrowsing(true)
    setBrowserFeedback(null)

    try {
      const result = await searchTargetFiles({
        targetProject: target,
        pattern,
        limit: 30,
        ...buildMcpConfig(),
      })

      setSearchResult(result)
      if (result.warnings.length > 0) {
        setBrowserFeedback(result.warnings.join('\n'))
      }
    } catch (error) {
      setBrowserFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsBrowsing(false)
    }
  }

  async function handleIndexProject(): Promise<void> {
    const target = targetProject.trim()
    if (!target) {
      setBrowserFeedback('Target project path is required.')
      return
    }

    setIsIndexing(true)
    setBrowserFeedback(null)

    try {
      const result = await indexTargetProject({ targetProject: target })
      setIndexResult(result)
      setSemanticResults([])
      setBrowserFeedback(
        `Indexed ${result.indexedFiles} files into ${result.indexedChunks} chunks (table: ${result.tableName}).`,
      )
    } catch (error) {
      setBrowserFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsIndexing(false)
    }
  }

  async function handleSemanticSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const target = targetProject.trim()
    const query = semanticQuery.trim()

    if (!target) {
      setBrowserFeedback('Target project path is required.')
      return
    }
    if (!query) {
      setBrowserFeedback('Semantic query is required.')
      return
    }

    setIsSemanticSearching(true)
    setBrowserFeedback(null)

    try {
      const results = await queryCodebase({
        targetProject: target,
        query,
        topK: 5,
      })
      setSemanticResults(results)
      if (results.length === 0) {
        setBrowserFeedback('No semantic chunks found. Run indexing first or broaden the query.')
      }
    } catch (error) {
      setBrowserFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsSemanticSearching(false)
    }
  }

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <h1 className="app-title">Autonomous Orchestration Platform</h1>
          <p className="app-subtitle">
            Phase 4 adds Tier 1 objective decomposition with PRA risk scoring and budget allocation.
          </p>
        </div>
        <strong>{taskCountLabel}</strong>
      </header>

      <section className="grid">
        <article className="card">
          <div className="card-header">
            <h2 className="card-title">Create Task</h2>
          </div>
          <div className="card-content">
            <form className="task-form" onSubmit={handleTaskSubmit}>
              <div className="field">
                <label htmlFor="tier">Tier</label>
                <select
                  id="tier"
                  value={formState.tier}
                  onChange={(event) =>
                    setFormState((current) => ({
                      ...current,
                      tier: Number(event.target.value) as 1 | 2 | 3,
                    }))
                  }
                >
                  <option value={1}>1 (Orchestrator)</option>
                  <option value={2}>2 (Domain Leader)</option>
                  <option value={3}>3 (Specialist)</option>
                </select>
              </div>

              <div className="field">
                <label htmlFor="domain">Domain</label>
                <input
                  id="domain"
                  value={formState.domain}
                  onChange={(event) =>
                    setFormState((current) => ({
                      ...current,
                      domain: event.target.value,
                    }))
                  }
                  placeholder="auth"
                />
              </div>

              <div className="field">
                <label htmlFor="token-budget">Token Budget</label>
                <input
                  id="token-budget"
                  type="number"
                  min={1}
                  value={formState.tokenBudget}
                  onChange={(event) =>
                    setFormState((current) => ({
                      ...current,
                      tokenBudget: Number(event.target.value || 0),
                    }))
                  }
                />
              </div>

              <div className="field">
                <label htmlFor="objective">Objective</label>
                <textarea
                  id="objective"
                  value={formState.objective}
                  onChange={(event) =>
                    setFormState((current) => ({
                      ...current,
                      objective: event.target.value,
                    }))
                  }
                  placeholder="Refactor auth module for lower re-render pressure."
                />
              </div>

              <button disabled={isSubmitting} type="submit">
                {isSubmitting ? 'Creating...' : 'Create Task'}
              </button>
            </form>

            {taskFeedback ? <p className="feedback">{taskFeedback}</p> : null}
          </div>
        </article>

        <article className="card">
          <div className="card-header">
            <h2 className="card-title">Task List</h2>
          </div>
          <div className="card-content">
            {isLoadingTasks ? <p className="empty-state">Loading tasks...</p> : null}

            {!isLoadingTasks && tasks.length === 0 ? (
              <p className="empty-state">No tasks yet. Create the first task from the form.</p>
            ) : null}

            {!isLoadingTasks && tasks.length > 0 ? (
              <ul className="task-list">
                {tasks.map((task) => (
                  <li className="task-item" key={task.id}>
                    <div className="task-row">
                      <span className="task-domain">{task.domain}</span>
                      <span className={cn('status-pill', STATUS_CLASS[task.status])}>
                        {task.status}
                      </span>
                    </div>

                    <p className="task-objective">{task.objective}</p>

                    <div className="task-meta">
                      <span>Tier {task.tier}</span>
                      <span>Budget {task.tokenBudget}</span>
                      <span>Created {formatTimestamp(task.createdAt)}</span>
                    </div>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </article>
      </section>

      <section className="card browser-card">
        <div className="card-header">
          <h2 className="card-title">Tier 1 Orchestrator</h2>
        </div>
        <div className="card-content">
          <form className="task-form" onSubmit={handleOrchestrateSubmit}>
            <div className="field">
              <label htmlFor="orchestrator-objective">Objective</label>
              <textarea
                id="orchestrator-objective"
                value={orchestratorObjective}
                onChange={(event) => setOrchestratorObjective(event.target.value)}
                placeholder="Refactor auth module"
              />
            </div>

            <div className="browser-inline-grid">
              <div className="field">
                <label htmlFor="orchestrator-budget">Global Token Budget</label>
                <input
                  id="orchestrator-budget"
                  min={100}
                  step={100}
                  type="number"
                  value={orchestratorBudget}
                  onChange={(event) => setOrchestratorBudget(Number(event.target.value || 0))}
                />
              </div>
              <div className="field">
                <label htmlFor="orchestrator-risk">Max Risk Tolerance (0.0 - 1.0)</label>
                <input
                  id="orchestrator-risk"
                  max={1}
                  min={0}
                  step={0.05}
                  type="number"
                  value={orchestratorRiskTolerance}
                  onChange={(event) => setOrchestratorRiskTolerance(Number(event.target.value || 0))}
                />
              </div>
            </div>

            <button disabled={isOrchestrating} type="submit">
              {isOrchestrating ? 'Orchestrating...' : 'Decompose Objective'}
            </button>
          </form>

          {orchestratorFeedback ? <p className="feedback">{orchestratorFeedback}</p> : null}

          {orchestrationResult ? (
            <div className="orchestration-result">
              <p className="meta-inline">
                Root task: {orchestrationResult.rootTask.id} | subtasks: {orchestrationResult.assignments.length} |
                distributed budget: {orchestrationResult.distributedBudget}
              </p>
              <ul className="orchestration-list">
                {orchestrationResult.assignments.map((assignment) => (
                  <li className="orchestration-item" key={assignment.taskId}>
                    <div className="task-row">
                      <span className="task-domain">{assignment.domain}</span>
                      <span className="meta-inline">
                        risk {assignment.riskFactor.toFixed(2)} ({assignmentRiskLabel(assignment)})
                      </span>
                    </div>
                    <p className="task-objective">{assignment.objective}</p>
                    <div className="task-meta">
                      <span>Budget {assignment.tokenBudget}</span>
                      <span>{assignment.relevantFiles.length} relevant files</span>
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      </section>

      <section className="card browser-card">
        <div className="card-header">
          <h2 className="card-title">Target Project Browser</h2>
        </div>
        <div className="card-content browser-content">
          <form
            className="browser-controls"
            onSubmit={(event) => {
              event.preventDefault()
              void browseDirectory('.')
            }}
          >
            <div className="field">
              <label htmlFor="target-project">Target Project Path</label>
              <input
                id="target-project"
                value={targetProject}
                onChange={(event) => setTargetProject(event.target.value)}
                placeholder="C:\\repo\\target-project"
              />
            </div>

            <div className="browser-inline-grid">
              <div className="field">
                <label htmlFor="mcp-command">MCP Command (optional)</label>
                <input
                  id="mcp-command"
                  value={mcpCommand}
                  onChange={(event) => setMcpCommand(event.target.value)}
                  placeholder="npx @aidd/mcp"
                />
              </div>
              <div className="field">
                <label htmlFor="mcp-args">MCP Args (optional)</label>
                <input
                  id="mcp-args"
                  value={mcpArgs}
                  onChange={(event) => setMcpArgs(event.target.value)}
                  placeholder="--project C:\\target"
                />
              </div>
            </div>

            <button disabled={isBrowsing} type="submit">
              {isBrowsing ? 'Working...' : 'Load Root'}
            </button>
          </form>

          <form className="browser-search" onSubmit={handleSearch}>
            <div className="field">
              <label htmlFor="search-pattern">Search Files</label>
              <input
                id="search-pattern"
                value={searchPattern}
                onChange={(event) => setSearchPattern(event.target.value)}
                placeholder="useSession"
              />
            </div>
            <button disabled={isBrowsing} type="submit">
              Search
            </button>
          </form>

          <div className="semantic-toolbar">
            <button disabled={isIndexing} type="button" onClick={() => void handleIndexProject()}>
              {isIndexing ? 'Indexing...' : 'Index Project'}
            </button>
            <span className="meta-inline">
              {indexResult
                ? `${indexResult.indexedFiles} files / ${indexResult.indexedChunks} chunks`
                : 'Run indexing before semantic query'}
            </span>
          </div>

          <form className="browser-search" onSubmit={handleSemanticSearch}>
            <div className="field">
              <label htmlFor="semantic-query">Semantic Query</label>
              <input
                id="semantic-query"
                value={semanticQuery}
                onChange={(event) => setSemanticQuery(event.target.value)}
                placeholder="components using session loading state"
              />
            </div>
            <button disabled={isSemanticSearching} type="submit">
              {isSemanticSearching ? 'Querying...' : 'Semantic Query'}
            </button>
          </form>

          {browserFeedback ? <p className="feedback">{browserFeedback}</p> : null}

          <div className="browser-grid">
            <div className="browser-panel">
              <div className="browser-panel-header">
                <strong>Directory</strong>
                <span className="meta-inline">{directory ? directory.cwd : '.'}</span>
                {directory?.source ? <span className="meta-inline">source: {directory.source}</span> : null}
              </div>

              <div className="browser-list">
                {directory?.parent ? (
                  <button
                    className="browser-entry"
                    type="button"
                    onClick={() => void browseDirectory(directory.parent ?? '.')}
                  >
                    <span className="browser-entry-icon">DIR</span>
                    <span>..</span>
                  </button>
                ) : null}

                {directory?.entries.map((entry) => (
                  <button
                    className="browser-entry"
                    key={entry.path}
                    type="button"
                    onClick={() => (entry.isDir ? void browseDirectory(entry.path) : void openFile(entry.path))}
                  >
                    <span className="browser-entry-icon">{entryIcon(entry)}</span>
                    <span>{entry.name}</span>
                  </button>
                ))}

                {!directory ? <p className="empty-state">Load a project path to browse files.</p> : null}
              </div>
            </div>

            <div className="browser-panel">
              <div className="browser-panel-header">
                <strong>File Preview</strong>
                <span className="meta-inline">{selectedFile?.path ?? 'No file selected'}</span>
              </div>

              <pre className="file-preview">
                {selectedFile ? selectedFile.content.slice(0, 5000) : 'Select a file to preview content.'}
              </pre>
            </div>

            <div className="browser-panel">
              <div className="browser-panel-header">
                <strong>Search Outputs</strong>
                <span className="meta-inline">
                  {searchResult ? `${searchResult.matches.length} file matches` : 'No search yet'}
                </span>
              </div>

              <ul className="search-list">
                {searchResult?.matches.map((match) => (
                  <li key={`${match.path}-${match.line ?? 0}`}>
                    <button className="search-match" type="button" onClick={() => void openFile(match.path)}>
                      <span>{match.path}</span>
                      {match.line ? <span className="meta-inline">line {match.line}</span> : null}
                    </button>
                  </li>
                ))}
              </ul>

              <div className="search-divider" />

              <ul className="search-list">
                {semanticResults.map((chunk) => (
                  <li key={chunk.id}>
                    <button className="search-match" type="button" onClick={() => void openFile(chunk.filePath)}>
                      <span>
                        {chunk.filePath}:{chunk.startLine}
                      </span>
                      <span className="meta-inline">score {chunk.score.toFixed(3)}</span>
                    </button>
                    <p className="chunk-preview">{chunk.content.slice(0, 140)}</p>
                  </li>
                ))}
                {semanticResults.length === 0 ? (
                  <li>
                    <p className="empty-state">No semantic query results yet.</p>
                  </li>
                ) : null}
              </ul>
            </div>
          </div>
        </div>
      </section>
    </main>
  )
}

export default App
