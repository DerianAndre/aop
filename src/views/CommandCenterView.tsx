import { useCallback, useEffect, useMemo, useState } from 'react'

import CommandBar from '@/components/command-center/CommandBar'
import InspectorPanel from '@/components/command-center/InspectorPanel'
import LiveFeedPanel from '@/components/command-center/LiveFeedPanel'
import PrimaryPanel from '@/components/command-center/PrimaryPanel'
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable'
import {
  approveOrchestrationPlan,
  getTasks,
  listTaskMutations,
  orchestrateObjective,
} from '@/hooks/useTauri'
import { useAopStore } from '@/store/aop-store'
import type { CcPhase } from '@/store/types'
import type { MutationRecord, TaskRecord } from '@/types'
import { toast } from 'sonner'

function derivePhase(
  rootTaskId: string | null,
  orchestrationResult: ReturnType<typeof useAopStore.getState>['ccOrchestrationResult'],
  tasks: Map<string, TaskRecord>,
  mutations: Map<string, MutationRecord>,
  isPlanning: boolean
): CcPhase {
  if (isPlanning) return 'planning'
  if (!rootTaskId) return 'empty'

  // If we have an orchestration result but haven't approved yet
  if (orchestrationResult && orchestrationResult.rootTask.id === rootTaskId) {
    const rootTask = tasks.get(rootTaskId)
    // If root is still pending and we have assignments, it's ready for approval
    if (rootTask?.status === 'pending' && orchestrationResult.assignments.length > 0) {
      return 'ready'
    }
  }

  const rootTask = tasks.get(rootTaskId)
  if (!rootTask) return 'empty'

  if (rootTask.status === 'completed') return 'completed'
  if (rootTask.status === 'failed') return 'failed'

  // Check if there are proposed mutations needing review
  const proposedMutations = Array.from(mutations.values()).filter(
    (m) => m.status === 'proposed'
  )
  if (proposedMutations.length > 0) return 'review'

  // If any child tasks are executing, we're running
  const childTasks = Array.from(tasks.values()).filter((t) => t.parentId === rootTaskId)
  if (childTasks.some((t) => t.status === 'executing' || t.status === 'pending')) {
    return 'running'
  }

  // If root is executing but no children yet, still running
  if (rootTask.status === 'executing') return 'running'

  return 'completed'
}

export function CommandCenterView() {
  const {
    tasks,
    mutations,
    ccRootTaskId,
    ccOrchestrationResult,
    ccInspectorItem,
    ccLiveFeedTab,
    ccInspectorCollapsed,
    ccLiveFeedCollapsed,
    targetProject,
    mcpCommand,
    mcpArgs,
    addTask,
    updateTask,
    addMutation,
    setCcRootTaskId,
    setCcOrchestrationResult,
    setCcInspectorItem,
    setCcLiveFeedTab,
    toggleCcLiveFeed,
    selectTask,
  } = useAopStore()

  const [isPlanning, setIsPlanning] = useState(false)
  const [isApproving, setIsApproving] = useState(false)

  // Derive phase from data
  const phase = useMemo(
    () => derivePhase(ccRootTaskId, ccOrchestrationResult, tasks, mutations, isPlanning),
    [ccRootTaskId, ccOrchestrationResult, tasks, mutations, isPlanning]
  )

  // Filtered task/mutation lists
  const taskList = useMemo(() => Array.from(tasks.values()), [tasks])
  const rootTasks = useMemo(
    () =>
      ccRootTaskId
        ? taskList.filter(
            (t) => t.id === ccRootTaskId || t.parentId === ccRootTaskId
          )
        : [],
    [taskList, ccRootTaskId]
  )
  const mutationList = useMemo(() => Array.from(mutations.values()), [mutations])

  const pendingMutationCount = useMemo(
    () => mutationList.filter((m) => m.status === 'proposed').length,
    [mutationList]
  )
  const errorCount = useMemo(
    () => rootTasks.filter((t) => t.status === 'failed').length,
    [rootTasks]
  )
  const [pendingBudgetCount] = useState(0)

  // Polling for tasks and mutations during active phases
  useEffect(() => {
    if (phase === 'empty' || phase === 'planning' || phase === 'ready') return

    let cancelled = false
    const poll = async () => {
      try {
        const freshTasks = await getTasks()
        if (cancelled) return
        for (const t of freshTasks) {
          const existing = tasks.get(t.id)
          if (!existing) {
            addTask(t)
          } else if (existing.updatedAt !== t.updatedAt) {
            updateTask(t.id, t)
          }
        }

        // Fetch mutations for all child tasks
        if (ccRootTaskId) {
          const childTaskIds = freshTasks
            .filter((t) => t.parentId === ccRootTaskId)
            .map((t) => t.id)

          for (const taskId of childTaskIds) {
            try {
              const taskMutations = await listTaskMutations({ taskId })
              for (const m of taskMutations) {
                const existing = mutations.get(m.id)
                if (!existing || existing.status !== m.status) {
                  addMutation(m)
                }
              }
            } catch {
              /* silent */
            }
          }
        }
      } catch {
        /* silent */
      }
    }

    poll()
    const interval = setInterval(poll, 1500)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [phase, ccRootTaskId])

  // Decompose objective
  const handleDecompose = useCallback(
    async (objective: string, budget: number, risk: number) => {
      if (!targetProject) {
        toast.error('Set a target project in settings first')
        return
      }

      setIsPlanning(true)
      try {
        const result = await orchestrateObjective({
          objective,
          targetProject,
          globalTokenBudget: budget,
          maxRiskTolerance: risk,
        })

        // Store root task and all assignments
        addTask(result.rootTask)
        for (const assignment of result.assignments) {
          addTask({
            id: assignment.taskId,
            parentId: assignment.parentId,
            tier: assignment.tier,
            domain: assignment.domain,
            objective: assignment.objective,
            status: 'pending',
            tokenBudget: assignment.tokenBudget,
            tokenUsage: 0,
            contextEfficiencyRatio: 0,
            riskFactor: assignment.riskFactor,
            complianceScore: 0,
            checksumBefore: null,
            checksumAfter: null,
            errorMessage: null,
            retryCount: 0,
            createdAt: Date.now() / 1000,
            updatedAt: Date.now() / 1000,
          })
        }

        setCcRootTaskId(result.rootTask.id)
        setCcOrchestrationResult(result)
        toast.success(`Plan generated: ${result.assignments.length} tasks`)
      } catch (err) {
        toast.error(`Planning failed: ${err}`)
      } finally {
        setIsPlanning(false)
      }
    },
    [targetProject, addTask, setCcRootTaskId, setCcOrchestrationResult]
  )

  // Approve plan
  const handleApprove = useCallback(async () => {
    if (!ccRootTaskId || !targetProject) return

    setIsApproving(true)
    try {
      const mcpArgsArray = mcpArgs ? mcpArgs.split(',').map((s) => s.trim()).filter(Boolean) : undefined

      await approveOrchestrationPlan({
        rootTaskId: ccRootTaskId,
        targetProject,
        mcpCommand: mcpCommand || undefined,
        mcpArgs: mcpArgsArray,
      })

      setCcOrchestrationResult(null)
      toast.success('Plan approved, execution started')
    } catch (err) {
      toast.error(`Execution failed: ${err}`)
    } finally {
      setIsApproving(false)
    }
  }, [ccRootTaskId, targetProject, mcpCommand, mcpArgs, setCcOrchestrationResult])

  // Cancel plan
  const handleCancelPlan = useCallback(() => {
    setCcRootTaskId(null)
    setCcOrchestrationResult(null)
  }, [setCcRootTaskId, setCcOrchestrationResult])

  // Inspector selection handlers
  const handleSelectTask = useCallback(
    (taskId: string) => {
      selectTask(taskId)
      setCcInspectorItem({ type: 'task', id: taskId })
    },
    [selectTask, setCcInspectorItem]
  )

  const handleSelectMutation = useCallback(
    (mutationId: string) => {
      setCcInspectorItem({ type: 'mutation', id: mutationId })
    },
    [setCcInspectorItem]
  )

  const handleCloseInspector = useCallback(() => {
    setCcInspectorItem(null)
  }, [setCcInspectorItem])

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Ctrl+K -> focus command bar (the textarea will handle this via autofocus)
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault()
        const textarea = document.querySelector<HTMLTextAreaElement>(
          '.command-center-view textarea'
        )
        textarea?.focus()
      }

      // Escape -> close inspector
      if (e.key === 'Escape') {
        if (ccInspectorItem) {
          handleCloseInspector()
        }
      }

      // Ctrl+B -> toggle live feed
      if ((e.ctrlKey || e.metaKey) && e.key === 'b') {
        e.preventDefault()
        toggleCcLiveFeed()
      }
    }

    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [ccInspectorItem, handleCloseInspector, toggleCcLiveFeed])

  return (
    <div className="command-center-view flex flex-col h-[calc(100vh-var(--header-height))]">
      {/* Zone 1: Command Bar */}
      <CommandBar
        phase={phase}
        tasks={rootTasks}
        pendingMutationCount={pendingMutationCount}
        pendingBudgetCount={pendingBudgetCount}
        errorCount={errorCount}
        isLoading={isPlanning || isApproving}
        onDecompose={handleDecompose}
        onApprove={handleApprove}
      />

      {/* Zone 2 + 3: Main Area + Live Feed */}
      <ResizablePanelGroup orientation="vertical" className="flex-1">
        {/* Main Area (Primary + Inspector) */}
        <ResizablePanel defaultSize={65} minSize={30}>
          {ccInspectorCollapsed || !ccInspectorItem ? (
            /* Full-width primary panel when inspector is hidden */
            <div className="h-full">
              <PrimaryPanel
                phase={phase}
                tasks={rootTasks}
                mutations={mutationList}
                orchestrationResult={ccOrchestrationResult}
                isApproving={isApproving}
                onApprove={handleApprove}
                onCancelPlan={handleCancelPlan}
                onSelectTask={handleSelectTask}
                onSelectMutation={handleSelectMutation}
              />
            </div>
          ) : (
            /* Split view when inspector is open */
            <ResizablePanelGroup orientation="horizontal">
              <ResizablePanel defaultSize={60} minSize={30}>
                <PrimaryPanel
                  phase={phase}
                  tasks={rootTasks}
                  mutations={mutationList}
                  orchestrationResult={ccOrchestrationResult}
                  isApproving={isApproving}
                  onApprove={handleApprove}
                  onCancelPlan={handleCancelPlan}
                  onSelectTask={handleSelectTask}
                  onSelectMutation={handleSelectMutation}
                />
              </ResizablePanel>
              <ResizableHandle withHandle />
              <ResizablePanel defaultSize={40} minSize={20}>
                <InspectorPanel
                  item={ccInspectorItem}
                  tasks={tasks}
                  mutations={mutations}
                  onClose={handleCloseInspector}
                />
              </ResizablePanel>
            </ResizablePanelGroup>
          )}
        </ResizablePanel>

        <ResizableHandle withHandle />

        {/* Live Feed */}
        <ResizablePanel
          defaultSize={35}
          minSize={5}
          collapsible
          collapsedSize={5}
        >
          <LiveFeedPanel
            rootTaskId={ccRootTaskId}
            tasks={rootTasks}
            errorCount={errorCount}
            pendingBudgetCount={pendingBudgetCount}
            isCollapsed={ccLiveFeedCollapsed}
            activeTab={ccLiveFeedTab}
            onTabChange={setCcLiveFeedTab}
            onToggleCollapse={toggleCcLiveFeed}
          />
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  )
}
