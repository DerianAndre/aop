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
import {
  analyzeObjective,
  approveOrchestrationPlan,
  controlTask,
  getTasks,
  orchestrateObjective,
  submitAnswersAndPlan,
} from '@/hooks/useTauri'
import { executeRestartApply, formatRestartApplyIssue } from '@/lib/restartApply'
import { useAopStore } from '@/store/aop-store'
import type { GeneratedPlan, ObjectiveAnalysis, OrchestrationResult, PlanExecutionResult, TaskControlAction, TaskRecord } from '@/types'

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
  const [isApprovingPlan, setIsApprovingPlan] = useState(false)
  const [planExecutionResult, setPlanExecutionResult] = useState<PlanExecutionResult | null>(null)

  // LLM analysis flow state
  const [isAnalyzing, setIsAnalyzing] = useState(false)
  const [analysisResult, setAnalysisResult] = useState<ObjectiveAnalysis | null>(null)
  const [userAnswers, setUserAnswers] = useState<Record<string, string>>({})
  const [isGeneratingPlan, setIsGeneratingPlan] = useState(false)
  const [generatedPlan, setGeneratedPlan] = useState<GeneratedPlan | null>(null)

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
      setPlanExecutionResult(null)
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

        const summary = await executeRestartApply({
          updatedTasks,
          targetProject: target,
          mcpConfig,
          topK: 8,
        })
        const restartIssue = formatRestartApplyIssue(summary)
        if (restartIssue) {
          setOrchestrationControlError(restartIssue)
        }
      }

      await loadTasks()
    } catch (error) {
      setOrchestrationControlError(error instanceof Error ? error.message : String(error))
    } finally {
      setActiveControlAction(null)
    }
  }

  async function handleApprovePlan() {
    const rootTaskId = orchestrationResult?.rootTask.id ?? monitoredTaskId
    if (!rootTaskId) {
      return
    }

    const target = targetProject.trim()
    if (!target) {
      setOrchestrationControlError('Target project path is required before approving the orchestration plan.')
      return
    }

    setOrchestrationControlError(null)
    setIsApprovingPlan(true)
    try {
      const result = await approveOrchestrationPlan({
        rootTaskId,
        targetProject: target,
        topK: 8,
        ...mcpConfig,
      })
      setPlanExecutionResult(result)
      if (result.failedExecutions > 0 || result.appliedMutations === 0) {
        setOrchestrationControlError(result.message)
      }
      await loadTasks()
    } catch (error) {
      setOrchestrationControlError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsApprovingPlan(false)
    }
  }

  async function handleAnalyzeObjective() {
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

    setOrchestrationError(null)
    setIsAnalyzing(true)
    setAnalysisResult(null)
    setGeneratedPlan(null)
    setUserAnswers({})
    try {
      const result = await analyzeObjective({
        objective: trimmedObjective,
        targetProject: target,
        globalTokenBudget: Math.floor(globalTokenBudget),
      })
      setAnalysisResult(result)
      const initialAnswers: Record<string, string> = {}
      result.questions.forEach((q, i) => {
        initialAnswers[`q${i}`] = ''
      })
      setUserAnswers(initialAnswers)
      await loadTasks()
    } catch (error) {
      setOrchestrationError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsAnalyzing(false)
    }
  }

  async function handleSubmitAnswersAndPlan() {
    if (!analysisResult) {
      return
    }
    const target = targetProject.trim()
    if (!target) {
      setOrchestrationError('Target project path is required.')
      return
    }

    const answersMap: Record<string, string> = {}
    analysisResult.questions.forEach((q, i) => {
      const answer = userAnswers[`q${i}`]?.trim()
      if (answer) {
        answersMap[q] = answer
      }
    })

    setOrchestrationError(null)
    setIsGeneratingPlan(true)
    try {
      const result = await submitAnswersAndPlan({
        rootTaskId: analysisResult.rootTaskId,
        objective: objective.trim(),
        answers: answersMap,
        targetProject: target,
        globalTokenBudget: Math.floor(globalTokenBudget),
        maxRiskTolerance: Number(maxRiskTolerance.toFixed(2)),
      })
      setGeneratedPlan(result)
      setOrchestrationResult({
        rootTask: result.rootTask,
        assignments: result.assignments,
        overheadBudget: result.overheadBudget,
        reserveBudget: result.reserveBudget,
        distributedBudget: result.distributedBudget,
      })
      setPlanExecutionResult(null)
      setObjective('')
      await loadTasks()
    } catch (error) {
      setOrchestrationError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsGeneratingPlan(false)
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
  const monitoredTaskId = orchestrationResult?.rootTask.id ?? analysisResult?.rootTaskId ?? executingTier1TaskId
  const canPauseMonitoredTask = monitoredTaskId ? pausableTaskIds.has(monitoredTaskId) : false
  const canResumeMonitoredTask = monitoredTaskId ? resumableTaskIds.has(monitoredTaskId) : false
  const canStopMonitoredTask = monitoredTaskId ? stoppableTaskIds.has(monitoredTaskId) : false
  const canRestartMonitoredTask = monitoredTaskId ? restartableTaskIds.has(monitoredTaskId) : false
  const monitoredTask = useMemo(
    () => tasks.find((task) => task.id === monitoredTaskId) ?? null,
    [monitoredTaskId, tasks],
  )
  const canApprovePlan = monitoredTask?.tier === 1 && monitoredTask.status === 'paused'

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

            <div className="flex gap-2">
              <Button disabled={isOrchestrating || isAnalyzing} type="submit">
                {isOrchestrating ? 'Orchestrating...' : 'Quick Decompose (Fast Path)'}
              </Button>
              <Button
                disabled={isOrchestrating || isAnalyzing}
                onClick={(e) => {
                  e.preventDefault()
                  void handleAnalyzeObjective()
                }}
                type="button"
                variant="secondary"
              >
                {isAnalyzing ? 'Analyzing...' : 'Analyze & Ask Questions (LLM)'}
              </Button>
            </div>
          </form>

          {isOrchestrating ? (
            <div className="space-y-3">
              <div className="rounded-md border border-blue-500/30 bg-blue-500/5 p-4">
                <div className="flex items-center gap-2">
                  <div className="size-2 animate-pulse rounded-full bg-blue-500" />
                  <h4 className="text-sm font-semibold">Quick Decompose in progress...</h4>
                </div>
                <p className="text-muted-foreground text-xs mt-1">
                  Selecting Tier 1 model, collecting source files, calling LLM to generate task plan, creating subtask assignments.
                </p>
              </div>
              {monitoredTaskId ? (
                <TaskActivityFeed taskId={monitoredTaskId} title="Live Orchestration Activity" pollMs={1000} />
              ) : null}
            </div>
          ) : null}

          {isAnalyzing ? (
            <div className="space-y-3">
              <div className="rounded-md border border-blue-500/30 bg-blue-500/5 p-4">
                <div className="flex items-center gap-2">
                  <div className="size-2 animate-pulse rounded-full bg-blue-500" />
                  <h4 className="text-sm font-semibold">Analyzing Objective...</h4>
                </div>
                <p className="text-muted-foreground text-xs">
                  Collecting source files, selecting Tier 1 model, calling LLM for analysis and clarifying questions.
                </p>
              </div>
              {monitoredTaskId ? (
                <TaskActivityFeed taskId={monitoredTaskId} title="Live Analysis Activity" pollMs={1000} />
              ) : null}
            </div>
          ) : null}

          {analysisResult && !generatedPlan ? (
            <div className="space-y-4 rounded-md border p-4">
              <div>
                <h4 className="text-sm font-semibold">LLM Analysis</h4>
                <p className="text-muted-foreground text-sm">{analysisResult.initialAnalysis}</p>
              </div>
              {analysisResult.suggestedApproach ? (
                <div>
                  <h4 className="text-sm font-semibold">Suggested Approach</h4>
                  <p className="text-muted-foreground text-sm">{analysisResult.suggestedApproach}</p>
                </div>
              ) : null}
              {analysisResult.fileTreeSummary ? (
                <details className="text-xs">
                  <summary className="cursor-pointer text-sm font-semibold">
                    Project File Tree
                  </summary>
                  <pre className="text-muted-foreground mt-2 max-h-40 overflow-auto whitespace-pre-wrap rounded bg-muted p-2 text-[11px]">
                    {analysisResult.fileTreeSummary}
                  </pre>
                </details>
              ) : null}
              {analysisResult.questions.length > 0 ? (
                <div className="space-y-3">
                  <h4 className="text-sm font-semibold">Clarifying Questions</h4>
                  {analysisResult.questions.map((question, idx) => (
                    <div className="space-y-1" key={idx}>
                      <Label className="text-sm" htmlFor={`analysis-q-${idx}`}>
                        {question}
                      </Label>
                      <Input
                        id={`analysis-q-${idx}`}
                        onChange={(e) =>
                          setUserAnswers((prev) => ({
                            ...prev,
                            [`q${idx}`]: e.target.value,
                          }))
                        }
                        placeholder="Your answer..."
                        value={userAnswers[`q${idx}`] ?? ''}
                      />
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-muted-foreground text-sm">No questions needed — objective is clear.</p>
              )}
              <Button
                disabled={isGeneratingPlan}
                onClick={() => void handleSubmitAnswersAndPlan()}
                type="button"
              >
                {isGeneratingPlan ? 'Generating Plan...' : 'Submit Answers & Generate Plan'}
              </Button>
            </div>
          ) : null}

          {isGeneratingPlan ? (
            <div className="space-y-3 rounded-md border border-blue-500/30 bg-blue-500/5 p-4">
              <div className="flex items-center gap-2">
                <div className="size-2 animate-pulse rounded-full bg-blue-500" />
                <h4 className="text-sm font-semibold">Generating Plan...</h4>
              </div>
              <p className="text-muted-foreground text-xs">
                LLM is processing your answers and generating a detailed task plan with assignments, budgets, and risk factors.
              </p>
            </div>
          ) : null}

          {generatedPlan?.riskAssessment ? (
            <div className="rounded-md border border-yellow-500/30 bg-yellow-500/5 p-3">
              <h4 className="text-sm font-semibold">Risk Assessment</h4>
              <p className="text-muted-foreground text-sm">{generatedPlan.riskAssessment}</p>
            </div>
          ) : null}

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
                      <strong>
                        Tier {assignment.tier} · {assignment.domain}
                      </strong>
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
                  disabled={isApprovingPlan || !canApprovePlan}
                  onClick={() => void handleApprovePlan()}
                  size="sm"
                  type="button"
                >
                  {isApprovingPlan ? 'Approving Plan...' : 'Approve Plan & Spawn Smart Agents'}
                </Button>
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

              {isApprovingPlan ? (
                <div className="space-y-3">
                  <div className="rounded-md border border-blue-500/30 bg-blue-500/5 p-3">
                    <div className="flex items-center gap-2">
                      <div className="size-2 animate-pulse rounded-full bg-blue-500" />
                      <h4 className="text-sm font-semibold">Executing Plan...</h4>
                    </div>
                    <p className="text-muted-foreground text-xs mt-1">
                      Spawning Tier 2 domain leaders and Tier 3 specialists. Calling LLM for code generation, computing diffs, running shadow tests.
                    </p>
                  </div>
                  {monitoredTaskId ? (
                    <TaskActivityFeed taskId={monitoredTaskId} title="Live Execution Log" pollMs={800} />
                  ) : null}
                </div>
              ) : null}

              {planExecutionResult ? (
                <div className={`rounded-md border p-3 ${planExecutionResult.failedExecutions > 0 && planExecutionResult.appliedMutations === 0 ? 'border-destructive/30 bg-destructive/5' : planExecutionResult.failedExecutions > 0 ? 'border-yellow-500/30 bg-yellow-500/5' : 'border-green-500/30 bg-green-500/5'}`}>
                  <p className="text-sm font-semibold mb-1">
                    {planExecutionResult.appliedMutations > 0
                      ? `${planExecutionResult.appliedMutations} mutation(s) applied`
                      : 'No mutations applied'}
                    {planExecutionResult.failedExecutions > 0
                      ? ` · ${planExecutionResult.failedExecutions} failed`
                      : ''}
                  </p>
                  <pre className="text-xs whitespace-pre-wrap max-h-48 overflow-auto text-muted-foreground">
                    {planExecutionResult.message}
                  </pre>
                </div>
              ) : null}

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
                  disabled={isApprovingPlan || !canApprovePlan}
                  onClick={() => void handleApprovePlan()}
                  size="sm"
                  type="button"
                >
                  {isApprovingPlan ? 'Approving Plan...' : 'Approve Plan & Spawn Smart Agents'}
                </Button>
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
