import { useEffect, useMemo, useState } from 'react'
import type { FormEvent } from 'react'

import { createTask, getTasks } from '@/hooks/useTauri'
import { cn } from '@/lib/utils'
import type { CreateTaskInput, TaskRecord, TaskStatus } from '@/types'

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

function App() {
  const [tasks, setTasks] = useState<TaskRecord[]>([])
  const [formState, setFormState] = useState<CreateTaskInput>(DEFAULT_FORM)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isLoading, setIsLoading] = useState(true)
  const [feedback, setFeedback] = useState<string | null>(null)

  const taskCountLabel = useMemo(() => {
    const count = tasks.length
    return `${count} task${count === 1 ? '' : 's'}`
  }, [tasks.length])

  useEffect(() => {
    void loadTasks()
  }, [])

  async function loadTasks() {
    setIsLoading(true)
    setFeedback(null)

    try {
      const records = await getTasks()
      setTasks(records)
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoading(false)
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setFeedback(null)

    if (!formState.objective.trim()) {
      setFeedback('Objective is required.')
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
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <h1 className="app-title">Autonomous Orchestration Platform</h1>
          <p className="app-subtitle">
            Phase 1 foundation: create tasks through Tauri IPC and persist them in SQLite.
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
            <form className="task-form" onSubmit={handleSubmit}>
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

            {feedback ? <p className="feedback">{feedback}</p> : null}
          </div>
        </article>

        <article className="card">
          <div className="card-header">
            <h2 className="card-title">Task List</h2>
          </div>
          <div className="card-content">
            {isLoading ? <p className="empty-state">Loading tasks...</p> : null}

            {!isLoading && tasks.length === 0 ? (
              <p className="empty-state">No tasks yet. Create the first task from the form.</p>
            ) : null}

            {!isLoading && tasks.length > 0 ? (
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
    </main>
  )
}

export default App
