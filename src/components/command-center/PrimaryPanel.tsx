import { useCallback } from 'react'

import {
  CheckCircle2,
  CircleDot,
  Crosshair,
  FileCode,
  GitPullRequest,
  Loader2,
  XCircle,
} from 'lucide-react'

import TaskGraph from '@/components/TaskGraph'
import PlanReviewCards from '@/components/command-center/PlanReviewCards'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import { useAopStore } from '@/store/aop-store'
import type { CcPhase } from '@/store/types'
import type { MutationRecord, OrchestrationResult, TaskRecord } from '@/types'

interface PrimaryPanelProps {
  phase: CcPhase
  tasks: TaskRecord[]
  mutations: MutationRecord[]
  orchestrationResult: OrchestrationResult | null
  isApproving: boolean
  onApprove: () => void
  onCancelPlan: () => void
  onSelectTask: (taskId: string) => void
  onSelectMutation: (mutationId: string) => void
}

const MUTATION_STATUS_ICON: Record<string, typeof CheckCircle2> = {
  proposed: CircleDot,
  validated: CheckCircle2,
  validated_no_tests: CheckCircle2,
  applied: CheckCircle2,
  rejected: XCircle,
}

const MUTATION_STATUS_CLASS: Record<string, string> = {
  proposed: 'text-blue-500',
  validated: 'text-green-500',
  validated_no_tests: 'text-yellow-500',
  applied: 'text-green-600',
  rejected: 'text-red-500',
}

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center p-8 gap-3">
      <Crosshair className="size-10 text-muted-foreground/40" />
      <div className="space-y-1">
        <h3 className="text-sm font-medium text-muted-foreground">No Active Orchestration</h3>
        <p className="text-xs text-muted-foreground/60 max-w-sm">
          Enter an objective in the command bar above to decompose it into tasks and start execution.
        </p>
      </div>
    </div>
  )
}

function PlanningState() {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 p-8">
      <Loader2 className="size-8 text-primary animate-spin" />
      <div className="space-y-1 text-center">
        <h3 className="text-sm font-medium">Analyzing Objective</h3>
        <p className="text-xs text-muted-foreground">
          Estimating complexity, allocating budget, scanning project files...
        </p>
      </div>
      <div className="space-y-2 w-48">
        <Skeleton className="h-2 w-full" />
        <Skeleton className="h-2 w-3/4" />
        <Skeleton className="h-2 w-1/2" />
      </div>
    </div>
  )
}

function MutationQueue({
  mutations,
  onSelect,
}: {
  mutations: MutationRecord[]
  onSelect: (id: string) => void
}) {
  const proposed = mutations.filter((m) => m.status === 'proposed')
  const others = mutations.filter((m) => m.status !== 'proposed')
  const sorted = [...proposed, ...others]

  return (
    <ScrollArea className="h-full">
      <div className="space-y-1 p-4">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-medium flex items-center gap-1.5">
            <GitPullRequest className="size-4" />
            Mutation Review
          </h3>
          <Badge variant="secondary" className="text-xs">
            {proposed.length} pending
          </Badge>
        </div>
        {sorted.map((mutation) => {
          const StatusIcon = MUTATION_STATUS_ICON[mutation.status] ?? CircleDot
          const statusClass = MUTATION_STATUS_CLASS[mutation.status] ?? 'text-muted-foreground'
          const fileName = mutation.filePath.split('/').pop() ?? mutation.filePath

          return (
            <button
              key={mutation.id}
              onClick={() => onSelect(mutation.id)}
              className="w-full text-left px-3 py-2 rounded-md hover:bg-muted/50 transition-colors flex items-start gap-2 group"
            >
              <StatusIcon className={`size-3.5 mt-0.5 shrink-0 ${statusClass}`} />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <FileCode className="size-3 text-muted-foreground shrink-0" />
                  <span className="text-xs font-mono truncate">{fileName}</span>
                  <Badge variant="outline" className="text-[10px] ml-auto shrink-0">
                    {Math.round(mutation.confidence * 100)}%
                  </Badge>
                </div>
                {mutation.intentDescription && (
                  <p className="text-[11px] text-muted-foreground truncate mt-0.5">
                    {mutation.intentDescription}
                  </p>
                )}
              </div>
            </button>
          )
        })}
        {sorted.length === 0 && (
          <p className="text-xs text-muted-foreground text-center py-6">
            No mutations yet. Waiting for agents to propose changes...
          </p>
        )}
      </div>
    </ScrollArea>
  )
}

function CompletedState({ tasks }: { tasks: TaskRecord[] }) {
  const completed = tasks.filter((t) => t.status === 'completed').length
  const failed = tasks.filter((t) => t.status === 'failed').length
  const totalTokens = tasks.reduce((sum, t) => sum + t.tokenUsage, 0)

  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 p-8">
      <CheckCircle2 className="size-10 text-green-500" />
      <div className="space-y-1 text-center">
        <h3 className="text-sm font-medium">Orchestration Complete</h3>
        <p className="text-xs text-muted-foreground">
          {completed} tasks completed, {failed} failed, {totalTokens.toLocaleString()} tokens used
        </p>
      </div>
    </div>
  )
}

function FailedState({ tasks }: { tasks: TaskRecord[] }) {
  const completed = tasks.filter((t) => t.status === 'completed').length
  const failed = tasks.filter((t) => t.status === 'failed').length
  const totalTokens = tasks.reduce((sum, t) => sum + t.tokenUsage, 0)
  const firstError = tasks.find((t) => t.status === 'failed')?.errorMessage

  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 p-8">
      <XCircle className="size-10 text-destructive" />
      <div className="space-y-1 text-center">
        <h3 className="text-sm font-medium">Orchestration Failed</h3>
        <p className="text-xs text-muted-foreground">
          {completed} completed, {failed} failed, {totalTokens.toLocaleString()} tokens used
        </p>
        {firstError ? (
          <p className="text-xs text-destructive/80 max-w-md mt-2">{firstError}</p>
        ) : null}
      </div>
    </div>
  )
}

export default function PrimaryPanel({
  phase,
  tasks,
  mutations,
  orchestrationResult,
  isApproving,
  onApprove,
  onCancelPlan,
  onSelectTask,
  onSelectMutation,
}: PrimaryPanelProps) {
  const { selectedTaskId } = useAopStore()

  const handleTaskClick = useCallback(
    (taskId: string) => onSelectTask(taskId),
    [onSelectTask]
  )

  const handleTaskDoubleClick = useCallback(
    (taskId: string) => onSelectTask(taskId),
    [onSelectTask]
  )

  if (phase === 'empty') return <EmptyState />
  if (phase === 'planning') return <PlanningState />

  if (phase === 'ready' && orchestrationResult) {
    return (
      <ScrollArea className="h-full">
        <PlanReviewCards
          result={orchestrationResult}
          onApprove={onApprove}
          onCancel={onCancelPlan}
          isApproving={isApproving}
        />
      </ScrollArea>
    )
  }

  if (phase === 'running') {
    return (
      <div className="h-full">
        <TaskGraph
          tasks={tasks}
          selectedTaskId={selectedTaskId}
          onTaskClick={handleTaskClick}
          onTaskDoubleClick={handleTaskDoubleClick}
        />
      </div>
    )
  }

  if (phase === 'review') {
    return (
      <MutationQueue mutations={mutations} onSelect={onSelectMutation} />
    )
  }

  if (phase === 'completed') {
    return <CompletedState tasks={tasks} />
  }

  if (phase === 'failed') {
    return <FailedState tasks={tasks} />
  }

  return <EmptyState />
}
