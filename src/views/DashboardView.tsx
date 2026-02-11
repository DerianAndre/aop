import { useCallback, useEffect, useMemo, useState, type FormEvent } from 'react'

import TaskActivityFeed from '@/components/TaskActivityFeed'
import TaskBudgetPanel from '@/components/TaskBudgetPanel'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import TokenBurnChart from '@/components/TokenBurnChart'
import { useTargetProjectConfig } from '@/hooks/useTargetProjectConfig'
import { controlTask, executeDomainTask, getTasks, orchestrateObjective } from '@/hooks/useTauri'
import { useAopStore } from '@/store/aop-store'
import type { OrchestrationResult, TaskControlAction, TaskRecord } from '@/types'

function formatNumber(value: number): string {
  return new Intl.NumberFormat().format(value)
}

function mapRiskLabel(risk: number): string {
  if (risk > 0.7) {
    return 'High'
  }
  if (risk >= 0.3) {
    return 'Medium'
  }
  return 'Low'
}

export function DashboardView() {
  const addTask = useAopStore((state) => state.addTask)
  const tasksMap = useAopStore((state) => state.tasks)
  const tasks = useMemo<TaskRecord[]>(
    () =>
      Array.from<TaskRecord>(tasksMap.values()).sort(
        (left: TaskRecord, right: TaskRecord) => right.createdAt - left.createdAt,
      ),
    [tasksMap],
  )
  const parentByTaskId = useMemo(() => {
    const result = new Map<string, string | null>()
    tasks.forEach((task) => {
      result.set(task.id, task.parentId)
    })
    return result
  }, [tasks])
  const resumableTaskIds = useMemo(() => {
    const result = new Set<string>()
    tasks.forEach((task) => {
      if (task.status !== 'paused') {
        return
      }

      let currentId: string | null = task.id
      while (currentId) {
        result.add(currentId)
        currentId = parentByTaskId.get(currentId) ?? null
      }
    })
    return result
  }, [parentByTaskId, tasks])
  const pausableTaskIds = useMemo(() => {
    const result = new Set<string>()
    tasks.forEach((task) => {
      if (task.status === 'completed' || task.status === 'failed' || task.status === 'paused') {
        return
      }

      let currentId: string | null = task.id
      while (currentId) {
        result.add(currentId)
        currentId = parentByTaskId.get(currentId) ?? null
      }
    })
    return result
  }, [parentByTaskId, tasks])
  const stoppableTaskIds = useMemo(() => {
    const result = new Set<string>()
    tasks.forEach((task) => {
      if (task.status === 'completed' || task.status === 'failed') {
        return
      }

      let currentId: string | null = task.id
      while (currentId) {
        result.add(currentId)
        currentId = parentByTaskId.get(currentId) ?? null
      }
    })
    return result
  }, [parentByTaskId, tasks])
  const restartableTaskIds = useMemo(() => {
    const result = new Set<string>()
    tasks.forEach((task) => {
      if (task.status !== 'failed' && task.status !== 'completed' && task.status !== 'paused') {
        return
      }

      let currentId: string | null = task.id
      while (currentId) {
        result.add(currentId)
        currentId = parentByTaskId.get(currentId) ?? null
      }
    })
    return result
  }, [parentByTaskId, tasks])

  const { targetProject, setTargetProject, mcpConfig } = useTargetProjectConfig()
  const [isLoadingTasks, setIsLoadingTasks] = useState(false)
  const [taskError, setTaskError] = useState<string | null>(null)

  const [objective, setObjective] = useState('')
  const [globalTokenBudget, setGlobalTokenBudget] = useState(12000)
  const [maxRiskTolerance, setMaxRiskTolerance] = useState(0.6)
  const [isOrchestrating, setIsOrchestrating] = useState(false)
  const [orchestrationError, setOrchestrationError] = useState<string | null>(null)
  const [orchestrationControlError, setOrchestrationControlError] = useState<string | null>(null)
  const [activeControlAction, setActiveControlAction] = useState<TaskControlAction | null>(null)
  const [orchestrationResult, setOrchestrationResult] = useState<OrchestrationResult | null>(null)

  const loadTasks = useCallback(async () => {
    setIsLoadingTasks(true)
    setTaskError(null)
    try {
      const fetchedTasks = await getTasks()
      fetchedTasks.forEach((task) => addTask(task))
    } catch (error) {
      setTaskError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoadingTasks(false)
    }
  }, [addTask])

  useEffect(() => {
    void loadTasks()
  }, [loadTasks])

  useEffect(() => {
    if (!isOrchestrating) {
      return
    }

    const intervalRef = setInterval(() => {
      void loadTasks()
    }, 1200)

    return () => clearInterval(intervalRef)
  }, [isOrchestrating, loadTasks])

  async function handleOrchestrate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setOrchestrationError(null)

    const target = targetProject.trim()
    const trimmedObjective = objective.trim()

    if (!target) {
      setOrchestrationError('Target project path is required.')
      return
    }
    if (!trimmedObjective) {
      setOrchestrationError('Objective is required.')
      return
    }
    if (!Number.isFinite(globalTokenBudget) || globalTokenBudget < 100) {
      setOrchestrationError('Global token budget must be at least 100.')
      return
    }
    if (!Number.isFinite(maxRiskTolerance) || maxRiskTolerance < 0 || maxRiskTolerance > 1) {
      setOrchestrationError('Max risk tolerance must be between 0.0 and 1.0.')
      return
    }

    setIsOrchestrating(true)
    try {
      const result = await orchestrateObjective({
        objective: trimmedObjective,
        targetProject: target,
        globalTokenBudget: Math.floor(globalTokenBudget),
        maxRiskTolerance: Number(maxRiskTolerance.toFixed(2)),
      })
      setObjective('')
      setOrchestrationResult(result)
      await loadTasks()
    } catch (error) {
      setOrchestrationError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsOrchestrating(false)
    }
  }

  async function handleOrchestrationControl(action: TaskControlAction) {
    if (!monitoredTaskId) {
      return
    }

    setOrchestrationControlError(null)
    setActiveControlAction(action)
    try {
      const updatedTasks = await controlTask({
        taskId: monitoredTaskId,
        action,
        includeDescendants: true,
        reason: action === 'stop' ? 'manual stop from dashboard' : undefined,
      })
      if (updatedTasks.length === 0) {
        setOrchestrationControlError(`No tasks were updated for action '${action}'.`)
      }
      updatedTasks.forEach((task) => addTask(task))

      if (action === 'restart') {
        const target = targetProject.trim()
        if (!target) {
          setOrchestrationControlError(
            'Tasks were restarted, but no target project is configured to run Tier 2 agents.',
          )
          await loadTasks()
          return
        }

        const tier2TaskIds = Array.from(new Set(updatedTasks.filter((task) => task.tier === 2).map((task) => task.id)))
        const executionResults = await Promise.allSettled(
          tier2TaskIds.map((taskId) =>
            executeDomainTask({
              taskId,
              targetProject: target,
              topK: 8,
              ...mcpConfig,
            }),
          ),
        )
        const failedExecutions = executionResults.filter((result) => result.status === 'rejected')
        if (failedExecutions.length > 0) {
          const firstFailure = failedExecutions[0] as PromiseRejectedResult
          const message =
            firstFailure.reason instanceof Error
              ? firstFailure.reason.message
              : String(firstFailure.reason)
          setOrchestrationControlError(
            `Restarted tasks, but ${failedExecutions.length} Tier 2 execution(s) failed. First error: ${message}`,
          )
        }
      }

      await loadTasks()
    } catch (error) {
      setOrchestrationControlError(error instanceof Error ? error.message : String(error))
    } finally {
      setActiveControlAction(null)
    }
  }

  const activeTasks = useMemo(
    () => tasks.filter((task) => task.status !== 'completed' && task.status !== 'failed').length,
    [tasks],
  )
  const executingTasks = useMemo(() => tasks.filter((task) => task.status === 'executing').length, [tasks])
  const totalTokensSpent = useMemo(() => tasks.reduce((total, task) => total + task.tokenUsage, 0), [tasks])
  const totalTokenBudget = useMemo(() => tasks.reduce((total, task) => total + task.tokenBudget, 0), [tasks])
  const avgCompliance = useMemo(() => {
    if (tasks.length === 0) {
      return 100
    }
    const total = tasks.reduce((sum, task) => sum + task.complianceScore, 0)
    return Math.max(0, Math.min(100, total / tasks.length))
  }, [tasks])
  const executingTier1TaskId = useMemo(
    () => tasks.find((task) => task.tier === 1 && task.status === 'executing')?.id ?? null,
    [tasks],
  )
  const monitoredTaskId = orchestrationResult?.rootTask.id ?? executingTier1TaskId
  const canPauseMonitoredTask = monitoredTaskId ? pausableTaskIds.has(monitoredTaskId) : false
  const canResumeMonitoredTask = monitoredTaskId ? resumableTaskIds.has(monitoredTaskId) : false
  const canStopMonitoredTask = monitoredTaskId ? stoppableTaskIds.has(monitoredTaskId) : false
  const canRestartMonitoredTask = monitoredTaskId ? restartableTaskIds.has(monitoredTaskId) : false
  const monitoredTask = useMemo(
    () => tasks.find((task) => task.id === monitoredTaskId) ?? null,
    [monitoredTaskId, tasks],
  )

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">Active Tasks</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">{formatNumber(activeTasks)}</div>
            <p className="text-muted-foreground text-xs">{executingTasks} executing</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">Tokens Spent</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">{formatNumber(totalTokensSpent)}</div>
            <p className="text-muted-foreground text-xs">of {formatNumber(totalTokenBudget)} budget</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">Avg Compliance</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">{avgCompliance.toFixed(1)}%</div>
            <p className="text-muted-foreground text-xs">
              {isLoadingTasks ? 'Refreshing...' : `${formatNumber(tasks.length)} tracked tasks`}
            </p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Token Burn Over Time</CardTitle>
          <Button onClick={() => void loadTasks()} size="sm" type="button" variant="outline">
            Refresh Tasks
          </Button>
        </CardHeader>
        <CardContent>
          {taskError ? <p className="text-destructive text-sm">{taskError}</p> : null}
          <TokenBurnChart tasks={tasks} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Tier 1 Orchestration</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <form className="space-y-4" onSubmit={handleOrchestrate}>
            <div className="space-y-2">
              <Label htmlFor="dashboard-target-project">Target Project Path</Label>
              <Input
                id="dashboard-target-project"
                onChange={(event) => setTargetProject(event.target.value)}
                placeholder="C:\\repo\\target-project"
                value={targetProject}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="dashboard-objective">Objective</Label>
              <Textarea
                id="dashboard-objective"
                onChange={(event) => setObjective(event.target.value)}
                placeholder="Refactor auth module for lower re-render pressure."
                value={objective}
              />
            </div>

            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="dashboard-budget">Global Token Budget</Label>
                <Input
                  id="dashboard-budget"
                  min={100}
                  onChange={(event) => setGlobalTokenBudget(Number(event.target.value || 0))}
                  step={100}
                  type="number"
                  value={globalTokenBudget}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="dashboard-risk">Max Risk Tolerance (0.0 - 1.0)</Label>
                <Input
                  id="dashboard-risk"
                  max={1}
                  min={0}
                  onChange={(event) => setMaxRiskTolerance(Number(event.target.value || 0))}
                  step={0.05}
                  type="number"
                  value={maxRiskTolerance}
                />
              </div>
            </div>

            {orchestrationError ? <p className="text-destructive text-sm">{orchestrationError}</p> : null}

            <Button disabled={isOrchestrating} type="submit">
              {isOrchestrating ? 'Orchestrating...' : 'Decompose Objective'}
            </Button>
          </form>

          {orchestrationResult ? (
            <div className="space-y-3 rounded-md border p-4">
              <p className="text-muted-foreground text-sm">
                Root task: <strong>{orchestrationResult.rootTask.id}</strong> | subtasks:{' '}
                <strong>{orchestrationResult.assignments.length}</strong> | distributed budget:{' '}
                <strong>{orchestrationResult.distributedBudget}</strong>
              </p>

              <div className="space-y-2">
                {orchestrationResult.assignments.map((assignment) => (
                  <div className="rounded-md border p-3" key={assignment.taskId}>
                    <div className="flex items-center justify-between gap-3">
                      <strong>{assignment.domain}</strong>
                      <span className="text-muted-foreground text-xs">
                        risk {assignment.riskFactor.toFixed(2)} ({mapRiskLabel(assignment.riskFactor)})
                      </span>
                    </div>
                    <p className="text-sm">{assignment.objective}</p>
                    <p className="text-muted-foreground text-xs">
                      Budget {assignment.tokenBudget} | Relevant files {assignment.relevantFiles.length}
                    </p>
                  </div>
                ))}
              </div>

              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={activeControlAction !== null || !canPauseMonitoredTask}
                  onClick={() => void handleOrchestrationControl('pause')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeControlAction === 'pause' ? 'Pausing...' : 'Pause T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !canResumeMonitoredTask}
                  onClick={() => void handleOrchestrationControl('resume')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeControlAction === 'resume' ? 'Resuming...' : 'Resume T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !canStopMonitoredTask}
                  onClick={() => void handleOrchestrationControl('stop')}
                  size="sm"
                  type="button"
                  variant="destructive"
                >
                  {activeControlAction === 'stop' ? 'Stopping...' : 'Stop T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !canRestartMonitoredTask}
                  onClick={() => void handleOrchestrationControl('restart')}
                  size="sm"
                  type="button"
                  variant="secondary"
                >
                  {activeControlAction === 'restart' ? 'Restarting...' : 'Restart T1/T2/T3'}
                </Button>
              </div>

              {orchestrationControlError ? (
                <p className="text-destructive text-sm whitespace-pre-wrap">{orchestrationControlError}</p>
              ) : null}
            </div>
          ) : null}

          <TaskBudgetPanel
            includeDescendants
            onChanged={async () => {
              await loadTasks()
            }}
            task={monitoredTask}
            title="Orchestration Budget Controls"
          />

          {monitoredTaskId ? (
            <TaskActivityFeed taskId={monitoredTaskId} title="Tier 1 + Tier 2 + Tier 3 Live Activity" />
          ) : null}

          {!orchestrationResult && monitoredTaskId ? (
            <div className="space-y-2">
              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={activeControlAction !== null || !canPauseMonitoredTask}
                  onClick={() => void handleOrchestrationControl('pause')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeControlAction === 'pause' ? 'Pausing...' : 'Pause T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !canResumeMonitoredTask}
                  onClick={() => void handleOrchestrationControl('resume')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeControlAction === 'resume' ? 'Resuming...' : 'Resume T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !canStopMonitoredTask}
                  onClick={() => void handleOrchestrationControl('stop')}
                  size="sm"
                  type="button"
                  variant="destructive"
                >
                  {activeControlAction === 'stop' ? 'Stopping...' : 'Stop T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !canRestartMonitoredTask}
                  onClick={() => void handleOrchestrationControl('restart')}
                  size="sm"
                  type="button"
                  variant="secondary"
                >
                  {activeControlAction === 'restart' ? 'Restarting...' : 'Restart T1/T2/T3'}
                </Button>
              </div>
              {orchestrationControlError ? (
                <p className="text-destructive text-sm whitespace-pre-wrap">{orchestrationControlError}</p>
              ) : null}
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  )
}
