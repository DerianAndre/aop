import { useCallback, useEffect, useMemo, useState } from 'react'

import ConflictResolutionPanel from '@/components/ConflictResolutionPanel'
import DiffReviewer from '@/components/DiffReviewer'
import TaskActivityFeed from '@/components/TaskActivityFeed'
import TaskBudgetPanel from '@/components/TaskBudgetPanel'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useTargetProjectConfig } from '@/hooks/useTargetProjectConfig'
import {
  controlTask,
  executeDomainTask,
  getTasks,
  listAuditLog,
  listTaskMutations,
  readTargetFile,
  requestMutationRevision,
  runMutationPipeline,
  setMutationStatus,
} from '@/hooks/useTauri'
import { useAopStore } from '@/store/aop-store'
import type {
  AuditLogEntry,
  IntentSummary,
  MutationPipelineResult,
  MutationRecord,
  PipelineStepResult,
  TaskControlAction,
  TaskRecord,
} from '@/types'

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000))
}

function mutationStatusVariant(status: string): 'secondary' | 'default' | 'destructive' | 'outline' {
  if (status === 'applied') return 'default'
  if (status === 'rejected') return 'destructive'
  if (status === 'validated' || status === 'validated_no_tests') return 'secondary'
  return 'outline'
}

function stepStatusVariant(status: string): 'secondary' | 'default' | 'destructive' | 'outline' {
  if (status === 'passed') return 'default'
  if (status === 'failed') return 'destructive'
  if (status === 'pending') return 'outline'
  return 'secondary'
}

export function MutationsView() {
  const addTask = useAopStore((state) => state.addTask)
  const selectedTaskId = useAopStore((state) => state.selectedTaskId)
  const selectTask = useAopStore((state) => state.selectTask)
  const tasksMap = useAopStore((state) => state.tasks)
  const tasks = useMemo<TaskRecord[]>(
    () =>
      Array.from<TaskRecord>(tasksMap.values()).sort(
        (left: TaskRecord, right: TaskRecord) => right.createdAt - left.createdAt,
      ),
    [tasksMap],
  )

  const { targetProject, setTargetProject, mcpConfig } = useTargetProjectConfig()

  const [isLoadingTasks, setIsLoadingTasks] = useState(false)
  const [isLoadingMutations, setIsLoadingMutations] = useState(false)
  const [isExecutingTier2, setIsExecutingTier2] = useState(false)
  const [isRunningPipeline, setIsRunningPipeline] = useState(false)
  const [isLoadingReviewer, setIsLoadingReviewer] = useState(false)
  const [isRequestingRevision, setIsRequestingRevision] = useState(false)

  const [tier2Summary, setTier2Summary] = useState<IntentSummary | null>(null)
  const [mutations, setMutations] = useState<MutationRecord[]>([])
  const [selectedMutationId, setSelectedMutationId] = useState<string | null>(null)
  const [selectedMutationOriginal, setSelectedMutationOriginal] = useState('')
  const [pipelineResult, setPipelineResult] = useState<MutationPipelineResult | null>(null)
  const [pipelineSteps, setPipelineSteps] = useState<PipelineStepResult[]>([])
  const [auditEntries, setAuditEntries] = useState<AuditLogEntry[]>([])
  const [feedback, setFeedback] = useState<string | null>(null)
  const [reviewerFeedback, setReviewerFeedback] = useState<string | null>(null)
  const [conflictFeedback, setConflictFeedback] = useState<string | null>(null)
  const [taskControlError, setTaskControlError] = useState<string | null>(null)
  const [activeTaskControl, setActiveTaskControl] = useState<TaskControlAction | null>(null)

  const selectedTask: TaskRecord | null =
    tasks.find((task) => task.id === selectedTaskId) ??
    null
  const selectedMutation: MutationRecord | null =
    mutations.find((mutation) => mutation.id === selectedMutationId) ??
    null

  const loadTasks = useCallback(async () => {
    setIsLoadingTasks(true)
    setFeedback(null)
    try {
      const fetchedTasks = await getTasks()
      fetchedTasks.forEach((task) => addTask(task))

      if (!selectedTaskId && fetchedTasks.length > 0) {
        selectTask(fetchedTasks[0].id)
      }
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoadingTasks(false)
    }
  }, [addTask, selectTask, selectedTaskId])

  const loadOriginalForMutation = useCallback(
    async (mutation: MutationRecord) => {
      const target = targetProject.trim()
      if (!target) {
        setSelectedMutationOriginal('')
        setReviewerFeedback('Set target project path to preview original file content.')
        return
      }

      setIsLoadingReviewer(true)
      setReviewerFeedback(null)
      try {
        const file = await readTargetFile({
          targetProject: target,
          filePath: mutation.filePath,
          ...mcpConfig,
        })
        setSelectedMutationOriginal(file.content)
      } catch (error) {
        setSelectedMutationOriginal('')
        setReviewerFeedback(error instanceof Error ? error.message : String(error))
      } finally {
        setIsLoadingReviewer(false)
      }
    },
    [mcpConfig, targetProject],
  )

  const loadMutationsForTask = useCallback(
    async (taskId: string, openReviewer: boolean) => {
      setIsLoadingMutations(true)
      setFeedback(null)
      try {
        const fetchedMutations = await listTaskMutations({ taskId })
        setMutations(fetchedMutations)

        if (fetchedMutations.length === 0) {
          setSelectedMutationId(null)
          setSelectedMutationOriginal('')
          return
        }

        const nextMutation =
          openReviewer
            ? fetchedMutations[0]
            : fetchedMutations.find((mutation) => mutation.id === selectedMutationId) ?? fetchedMutations[0]

        setSelectedMutationId(nextMutation.id)
        await loadOriginalForMutation(nextMutation)
      } catch (error) {
        setFeedback(error instanceof Error ? error.message : String(error))
      } finally {
        setIsLoadingMutations(false)
      }
    },
    [loadOriginalForMutation, selectedMutationId],
  )

  useEffect(() => {
    void loadTasks()
  }, [loadTasks])

  useEffect(() => {
    if (!selectedTaskId) {
      setMutations([])
      setSelectedMutationId(null)
      setSelectedMutationOriginal('')
      setTier2Summary(null)
      return
    }

    void loadMutationsForTask(selectedTaskId, false)
  }, [loadMutationsForTask, selectedTaskId])

  async function handleExecuteTier2Task(task: TaskRecord) {
    const target = targetProject.trim()
    if (!target) {
      setFeedback('Target project path is required before running Tier 2.')
      return
    }
    if (task.tier !== 2) {
      setFeedback('Only Tier 2 tasks can be executed in this panel.')
      return
    }

    setIsExecutingTier2(true)
    setFeedback(null)
    try {
      const summary = await executeDomainTask({
        taskId: task.id,
        targetProject: target,
        topK: 8,
        ...mcpConfig,
      })
      setTier2Summary(summary)
      await loadMutationsForTask(task.id, true)
      await loadTasks()
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsExecutingTier2(false)
    }
  }

  async function handleRunPipeline(mutationId: string, tier1Approved: boolean) {
    const target = targetProject.trim()
    if (!target) {
      setFeedback('Target project path is required before running mutation pipeline.')
      return
    }

    setIsRunningPipeline(true)
    setFeedback(null)
    try {
      const result = await runMutationPipeline({
        mutationId,
        targetProject: target,
        tier1Approved,
      })
      setPipelineResult(result)
      setPipelineSteps(result.steps)
      const audit = await listAuditLog({ targetId: mutationId, limit: 50 })
      setAuditEntries(audit)

      if (selectedTaskId) {
        await loadMutationsForTask(selectedTaskId, false)
      }
      await loadTasks()
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsRunningPipeline(false)
    }
  }

  async function handleSelectMutation(mutation: MutationRecord) {
    setSelectedMutationId(mutation.id)
    await loadOriginalForMutation(mutation)
  }

  async function handleApproveMutation(mutationId: string) {
    await handleRunPipeline(mutationId, true)
  }

  async function handleRejectMutation(mutationId: string, reason: string) {
    setReviewerFeedback(null)
    try {
      await setMutationStatus({
        mutationId,
        status: 'rejected',
        rejectionReason: reason,
        rejectedAtStep: 'diff_reviewer',
      })

      if (selectedTaskId) {
        await loadMutationsForTask(selectedTaskId, false)
      }
      const audit = await listAuditLog({ targetId: mutationId, limit: 50 })
      setAuditEntries(audit)
    } catch (error) {
      setReviewerFeedback(error instanceof Error ? error.message : String(error))
    }
  }

  async function handleRequestRevision(mutationId: string, note: string) {
    setReviewerFeedback(null)
    setIsRequestingRevision(true)
    try {
      const result = await requestMutationRevision({
        mutationId,
        note: note.trim(),
      })
      setReviewerFeedback(
        `Revision created: task ${result.revisedTask.id.slice(0, 8)} / mutation ${result.revisedMutation.id.slice(0, 8)}.`,
      )
      selectTask(result.revisedTask.id)
      await loadTasks()
      await loadMutationsForTask(result.revisedTask.id, true)

      const audit = await listAuditLog({ targetId: result.originalMutation.id, limit: 50 })
      setAuditEntries(audit)
    } catch (error) {
      setReviewerFeedback(error instanceof Error ? error.message : String(error))
    } finally {
      setIsRequestingRevision(false)
    }
  }

  async function handleConflictAccept(agentUid: string) {
    setConflictFeedback(null)
    const mutation = mutations.find((item) => item.agentUid === agentUid)
    if (!mutation) {
      setConflictFeedback('Unable to locate mutation for selected proposal.')
      return
    }
    await handleRunPipeline(mutation.id, true)
  }

  async function handleConflictRejectBoth(agentUidA: string, agentUidB: string) {
    setConflictFeedback(null)
    const mutationA = mutations.find((item) => item.agentUid === agentUidA)
    const mutationB = mutations.find((item) => item.agentUid === agentUidB)
    if (!mutationA || !mutationB) {
      setConflictFeedback('Unable to locate both mutations for rejection.')
      return
    }

    try {
      await Promise.all([
        setMutationStatus({
          mutationId: mutationA.id,
          status: 'rejected',
          rejectionReason: 'Conflict resolution rejected both proposals.',
          rejectedAtStep: 'conflict_resolution',
        }),
        setMutationStatus({
          mutationId: mutationB.id,
          status: 'rejected',
          rejectionReason: 'Conflict resolution rejected both proposals.',
          rejectedAtStep: 'conflict_resolution',
        }),
      ])
      if (selectedTaskId) {
        await loadMutationsForTask(selectedTaskId, false)
      }
      setConflictFeedback('Both proposals were rejected.')
    } catch (error) {
      setConflictFeedback(error instanceof Error ? error.message : String(error))
    }
  }

  function handleConflictMergeManually() {
    const mutation = mutations[0]
    if (!mutation) {
      setConflictFeedback('No mutation available for manual merge.')
      return
    }
    setSelectedMutationId(mutation.id)
    setConflictFeedback('Manual merge selected. Review and edit patch context in Diff Reviewer.')
    void loadOriginalForMutation(mutation)
  }

  async function handleTaskControl(action: TaskControlAction) {
    if (!selectedTask) {
      return
    }

    setTaskControlError(null)
    setActiveTaskControl(action)
    try {
      const updated = await controlTask({
        taskId: selectedTask.id,
        action,
        includeDescendants: true,
        reason: action === 'stop' ? 'manual stop from mutations panel' : undefined,
      })
      updated.forEach((task) => addTask(task))
      await loadTasks()
      if (selectedTaskId) {
        await loadMutationsForTask(selectedTaskId, false)
      }
    } catch (error) {
      setTaskControlError(error instanceof Error ? error.message : String(error))
    } finally {
      setActiveTaskControl(null)
    }
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Mutation Pipeline Control</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-[1fr_220px_auto]">
            <div className="space-y-2">
              <Label htmlFor="mutations-target-project">Target Project Path</Label>
              <Input
                id="mutations-target-project"
                onChange={(event) => setTargetProject(event.target.value)}
                placeholder="C:\\repo\\target-project"
                value={targetProject}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="mutations-task-select">Task</Label>
              <Select
                onValueChange={(value) => {
                  selectTask(value)
                }}
                value={selectedTaskId ?? undefined}
              >
                <SelectTrigger id="mutations-task-select" className="w-full">
                  <SelectValue placeholder="Select task" />
                </SelectTrigger>
                <SelectContent>
                  {tasks.map((task) => (
                    <SelectItem key={task.id} value={task.id}>
                      Tier {task.tier} · {task.domain} · {task.id.slice(0, 8)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-end gap-2">
              <Button disabled={isLoadingTasks} onClick={() => void loadTasks()} type="button" variant="outline">
                {isLoadingTasks ? 'Loading...' : 'Refresh Tasks'}
              </Button>
              <Button
                disabled={!selectedTaskId || isLoadingMutations}
                onClick={() => selectedTaskId && void loadMutationsForTask(selectedTaskId, false)}
                type="button"
                variant="outline"
              >
                {isLoadingMutations ? 'Loading...' : 'Load Mutations'}
              </Button>
            </div>
          </div>

          {selectedTask ? (
            <div className="space-y-3">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline">Tier {selectedTask.tier}</Badge>
                <Badge variant="outline">{selectedTask.domain}</Badge>
                <Badge variant="outline">{selectedTask.status}</Badge>
                <span className="text-muted-foreground text-sm">
                  tokens {selectedTask.tokenUsage}/{selectedTask.tokenBudget}
                </span>
                {selectedTask.tier === 2 ? (
                  <Button
                    disabled={isExecutingTier2}
                    onClick={() => void handleExecuteTier2Task(selectedTask)}
                    size="sm"
                    type="button"
                  >
                    {isExecutingTier2 ? 'Running Tier 2...' : 'Execute Tier 2'}
                  </Button>
                ) : null}
              </div>

              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={activeTaskControl !== null}
                  onClick={() => void handleTaskControl('pause')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeTaskControl === 'pause' ? 'Pausing...' : 'Pause T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeTaskControl !== null}
                  onClick={() => void handleTaskControl('resume')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeTaskControl === 'resume' ? 'Resuming...' : 'Resume T1/T2/T3'}
                </Button>
                <Button
                  disabled={activeTaskControl !== null}
                  onClick={() => void handleTaskControl('stop')}
                  size="sm"
                  type="button"
                  variant="destructive"
                >
                  {activeTaskControl === 'stop' ? 'Stopping...' : 'Stop T1/T2/T3'}
                </Button>
              </div>

              {taskControlError ? (
                <p className="text-destructive text-xs whitespace-pre-wrap">{taskControlError}</p>
              ) : null}
            </div>
          ) : null}

          {feedback ? <p className="text-muted-foreground text-sm whitespace-pre-wrap">{feedback}</p> : null}
          <TaskBudgetPanel
            includeDescendants
            onChanged={async () => {
              await loadTasks()
              if (selectedTaskId) {
                await loadMutationsForTask(selectedTaskId, false)
              }
            }}
            task={selectedTask}
            title="Task Budget and Requests"
          />
          <TaskActivityFeed taskId={selectedTaskId} title="Task Activity (Tier 1/2/3)" />
        </CardContent>
      </Card>

      {tier2Summary ? (
        <Card>
          <CardHeader>
            <CardTitle>Tier 2 Summary</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex flex-wrap gap-2">
              <Badge variant="outline">Task {tier2Summary.taskId}</Badge>
              <Badge variant="outline">Domain {tier2Summary.domain}</Badge>
              <Badge variant="outline">Status {tier2Summary.status}</Badge>
              <Badge variant="outline">Compliance {tier2Summary.complianceScore}</Badge>
              <Badge variant="outline">Tokens {tier2Summary.tokensSpent}</Badge>
            </div>
            <p className="text-sm">{tier2Summary.summary}</p>
            {tier2Summary.conflicts ? (
              <p className="text-sm">
                Conflict: {tier2Summary.conflicts.description} (distance{' '}
                {tier2Summary.conflicts.semanticDistance.toFixed(3)})
              </p>
            ) : null}
          </CardContent>
        </Card>
      ) : null}

      <ConflictResolutionPanel
        conflict={tier2Summary?.conflicts}
        mutations={mutations}
        proposals={tier2Summary?.proposals ?? []}
        onAcceptProposal={handleConflictAccept}
        onMergeManually={handleConflictMergeManually}
        onRejectBoth={handleConflictRejectBoth}
      />
      {conflictFeedback ? <p className="text-muted-foreground text-sm">{conflictFeedback}</p> : null}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[320px_1fr]">
        <Card>
          <CardHeader>
            <CardTitle>Approval Queue</CardTitle>
          </CardHeader>
          <CardContent>
            <ScrollArea className="h-[560px]">
              <div className="space-y-2">
                {mutations.map((mutation) => (
                  <div className="rounded-md border p-3" key={mutation.id}>
                    <div className="flex items-center justify-between gap-2">
                      <strong className="text-sm">{mutation.filePath}</strong>
                      <Badge variant={mutationStatusVariant(mutation.status)}>{mutation.status}</Badge>
                    </div>
                    <p className="text-muted-foreground text-xs">
                      mutation {mutation.id.slice(0, 8)} | confidence {mutation.confidence.toFixed(2)}
                    </p>
                    <div className="mt-2 flex flex-wrap gap-2">
                      <Button onClick={() => void handleSelectMutation(mutation)} size="sm" type="button" variant="outline">
                        Open Reviewer
                      </Button>
                      <Button
                        disabled={isRunningPipeline}
                        onClick={() => void handleRunPipeline(mutation.id, false)}
                        size="sm"
                        type="button"
                        variant="outline"
                      >
                        Validate
                      </Button>
                      <Button
                        disabled={isRunningPipeline}
                        onClick={() => void handleRunPipeline(mutation.id, true)}
                        size="sm"
                        type="button"
                      >
                        Apply
                      </Button>
                    </div>
                  </div>
                ))}
                {mutations.length === 0 ? <p className="text-muted-foreground text-sm">No mutations for selected task.</p> : null}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Diff Reviewer</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {reviewerFeedback ? <p className="text-muted-foreground text-sm whitespace-pre-wrap">{reviewerFeedback}</p> : null}
            <DiffReviewer
              isBusy={isRunningPipeline || isLoadingReviewer || isRequestingRevision}
              mutation={selectedMutation}
              onApprove={handleApproveMutation}
              onReject={handleRejectMutation}
              onRequestRevision={handleRequestRevision}
              originalContent={selectedMutationOriginal}
            />
          </CardContent>
        </Card>
      </div>

      {pipelineResult ? (
        <Card>
          <CardHeader>
            <CardTitle>Pipeline Result</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex flex-wrap gap-2">
              <Badge variant="outline">Task {pipelineResult.task.id}</Badge>
              <Badge variant="outline">Mutation {pipelineResult.mutation.id.slice(0, 8)}</Badge>
              <Badge variant={mutationStatusVariant(pipelineResult.mutation.status)}>
                {pipelineResult.mutation.status}
              </Badge>
              {pipelineResult.shadowDir ? <Badge variant="outline">Shadow {pipelineResult.shadowDir}</Badge> : null}
            </div>

            <div className="space-y-2">
              {pipelineSteps.map((step) => (
                <div className="flex items-center justify-between rounded-md border p-2" key={step.step}>
                  <div>
                    <strong className="text-sm">{step.step}</strong>
                    <p className="text-muted-foreground text-xs">{step.details}</p>
                  </div>
                  <Badge variant={stepStatusVariant(step.status)}>{step.status}</Badge>
                </div>
              ))}
            </div>

            <div className="space-y-2">
              <p className="text-sm font-semibold">Audit Trail ({auditEntries.length})</p>
              <ScrollArea className="h-[220px]">
                <div className="space-y-2">
                  {auditEntries.map((entry) => (
                    <div className="rounded-md border p-2 text-xs" key={entry.id}>
                      <div className="flex items-center justify-between gap-2">
                        <strong>{entry.action}</strong>
                        <span className="text-muted-foreground">{formatTimestamp(entry.timestamp)}</span>
                      </div>
                      {entry.details ? <p className="text-muted-foreground mt-1 whitespace-pre-wrap">{entry.details}</p> : null}
                    </div>
                  ))}
                </div>
              </ScrollArea>
            </div>
          </CardContent>
        </Card>
      ) : null}
    </div>
  )
}
