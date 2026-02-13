import { useCallback, useEffect, useState } from 'react'

import {
  ChevronRight,
  Pause,
  Play,
  RotateCcw,
  Square,
  X,
} from 'lucide-react'

import TaskBudgetPanel from '@/components/TaskBudgetPanel'
import DiffReviewer from '@/components/DiffReviewer'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import {
  controlTask,
  readTargetFile,
  runMutationPipeline,
  setMutationStatus,
  requestMutationRevision,
  listTerminalEvents,
} from '@/hooks/useTauri'
import { useAopStore } from '@/store/aop-store'
import type { CcInspectorItem } from '@/store/types'
import type { MutationRecord, TaskRecord, TerminalEventRecord } from '@/types'
import { toast } from 'sonner'

interface InspectorPanelProps {
  item: CcInspectorItem | null
  tasks: Map<string, TaskRecord>
  mutations: Map<string, MutationRecord>
  onClose: () => void
}

const STATUS_COLORS: Record<string, string> = {
  pending: 'bg-muted text-muted-foreground',
  executing: 'bg-blue-500/10 text-blue-600',
  completed: 'bg-green-500/10 text-green-600',
  failed: 'bg-red-500/10 text-red-600',
  paused: 'bg-yellow-500/10 text-yellow-600',
}

function TaskInspector({ task }: { task: TaskRecord }) {
  const [isBusy, setIsBusy] = useState(false)

  const handleControl = useCallback(
    async (action: 'pause' | 'resume' | 'stop' | 'restart') => {
      setIsBusy(true)
      try {
        await controlTask({ taskId: task.id, action, includeDescendants: true })
        toast.success(`Task ${action}ed`)
      } catch (err) {
        toast.error(`Failed to ${action}: ${err}`)
      } finally {
        setIsBusy(false)
      }
    },
    [task.id]
  )

  const budgetPct = task.tokenBudget > 0 ? Math.round((task.tokenUsage / task.tokenBudget) * 100) : 0

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Badge variant="outline" className={STATUS_COLORS[task.status]}>
            {task.status}
          </Badge>
          <Badge variant="secondary" className="text-[10px]">
            Tier {task.tier}
          </Badge>
          <Badge variant="outline" className="text-[10px]">
            {task.domain}
          </Badge>
        </div>
        <p className="text-sm">{task.objective}</p>
      </div>

      {/* Controls */}
      <div className="flex gap-1">
        {task.status === 'executing' && (
          <Button variant="outline" size="sm" disabled={isBusy} onClick={() => handleControl('pause')}>
            <Pause className="size-3 mr-1" /> Pause
          </Button>
        )}
        {task.status === 'paused' && (
          <Button variant="outline" size="sm" disabled={isBusy} onClick={() => handleControl('resume')}>
            <Play className="size-3 mr-1" /> Resume
          </Button>
        )}
        {(task.status === 'executing' || task.status === 'paused') && (
          <Button variant="outline" size="sm" disabled={isBusy} onClick={() => handleControl('stop')}>
            <Square className="size-3 mr-1" /> Stop
          </Button>
        )}
        {(task.status === 'failed' || task.status === 'completed') && (
          <Button variant="outline" size="sm" disabled={isBusy} onClick={() => handleControl('restart')}>
            <RotateCcw className="size-3 mr-1" /> Restart
          </Button>
        )}
      </div>

      <Separator />

      {/* Stats */}
      <div className="grid grid-cols-2 gap-3 text-xs">
        <div>
          <span className="text-muted-foreground">Tokens</span>
          <div className="font-mono tabular-nums">{task.tokenUsage.toLocaleString()} / {task.tokenBudget.toLocaleString()}</div>
          <div className="h-1 bg-muted rounded-full mt-1 overflow-hidden">
            <div
              className={`h-full rounded-full ${budgetPct > 90 ? 'bg-red-500' : budgetPct > 70 ? 'bg-yellow-500' : 'bg-primary'}`}
              style={{ width: `${Math.min(budgetPct, 100)}%` }}
            />
          </div>
        </div>
        <div>
          <span className="text-muted-foreground">Compliance</span>
          <div className="font-mono tabular-nums">{task.complianceScore}/100</div>
        </div>
        <div>
          <span className="text-muted-foreground">Risk</span>
          <div className="font-mono tabular-nums">{Math.round(task.riskFactor * 100)}%</div>
        </div>
        <div>
          <span className="text-muted-foreground">Retries</span>
          <div className="font-mono tabular-nums">{task.retryCount}</div>
        </div>
      </div>

      {task.errorMessage && (
        <>
          <Separator />
          <div className="text-xs">
            <span className="text-muted-foreground">Error</span>
            <p className="text-red-500 mt-1 font-mono text-[11px]">{task.errorMessage}</p>
          </div>
        </>
      )}

      <Separator />

      {/* Budget Panel */}
      <TaskBudgetPanel task={task} title="Budget" includeDescendants />
    </div>
  )
}

function MutationInspector({ mutation }: { mutation: MutationRecord }) {
  const { targetProject, mcpCommand, mcpArgs } = useAopStore()
  const [originalContent, setOriginalContent] = useState('')
  const [isBusy, setIsBusy] = useState(false)

  useEffect(() => {
    if (!mutation.filePath || !targetProject) return
    readTargetFile({
      targetProject,
      filePath: mutation.filePath,
      mcpCommand: mcpCommand || undefined,
      mcpArgs: mcpArgs ? mcpArgs.split(',') : undefined,
    })
      .then((result) => setOriginalContent(result.content))
      .catch(() => setOriginalContent(''))
  }, [mutation.filePath, targetProject, mcpCommand, mcpArgs])

  const handleApprove = useCallback(
    async (mutationId: string) => {
      setIsBusy(true)
      try {
        await runMutationPipeline({
          mutationId,
          targetProject,
          tier1Approved: true,
        })
        toast.success('Mutation approved and applied')
      } catch (err) {
        toast.error(`Failed to apply: ${err}`)
      } finally {
        setIsBusy(false)
      }
    },
    [targetProject]
  )

  const handleReject = useCallback(
    async (mutationId: string, reason: string) => {
      setIsBusy(true)
      try {
        await setMutationStatus({
          mutationId,
          status: 'rejected',
          rejectionReason: reason,
          rejectedAtStep: 'human_review',
        })
        toast.success('Mutation rejected')
      } catch (err) {
        toast.error(`Failed to reject: ${err}`)
      } finally {
        setIsBusy(false)
      }
    },
    []
  )

  const handleRevision = useCallback(
    async (mutationId: string, note: string) => {
      setIsBusy(true)
      try {
        await requestMutationRevision({ mutationId, note })
        toast.success('Revision requested')
      } catch (err) {
        toast.error(`Failed to request revision: ${err}`)
      } finally {
        setIsBusy(false)
      }
    },
    []
  )

  return (
    <DiffReviewer
      mutation={mutation}
      originalContent={originalContent}
      onApprove={handleApprove}
      onReject={handleReject}
      onRequestRevision={handleRevision}
      isBusy={isBusy}
    />
  )
}

function TerminalInspector({ taskId, actor }: { taskId: string; actor: string }) {
  const [events, setEvents] = useState<TerminalEventRecord[]>([])

  useEffect(() => {
    let cancelled = false
    const poll = async () => {
      try {
        const result = await listTerminalEvents({ actor, taskId, limit: 100 })
        if (!cancelled) setEvents(result)
      } catch {
        // silently fail
      }
    }
    poll()
    const interval = setInterval(poll, 2000)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [taskId, actor])

  return (
    <div className="space-y-1 font-mono text-[11px]">
      {events.map((ev) => (
        <div key={ev.id} className="flex gap-2 py-0.5">
          <span className="text-muted-foreground shrink-0 tabular-nums">
            {new Date(ev.timestamp * 1000).toLocaleTimeString()}
          </span>
          <span className="text-primary/80">{ev.action}</span>
          {ev.details && <span className="text-muted-foreground truncate">{ev.details}</span>}
        </div>
      ))}
      {events.length === 0 && (
        <p className="text-muted-foreground text-xs">No terminal events yet.</p>
      )}
    </div>
  )
}

function EmptyInspector() {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-2 text-center p-6">
      <ChevronRight className="size-6 text-muted-foreground/30" />
      <p className="text-xs text-muted-foreground/60">
        Select a task or mutation to inspect
      </p>
    </div>
  )
}

export default function InspectorPanel({
  item,
  tasks,
  mutations,
  onClose,
}: InspectorPanelProps) {
  if (!item) return <EmptyInspector />

  const content = (() => {
    if (item.type === 'task') {
      const task = tasks.get(item.id)
      if (!task) return <EmptyInspector />
      return <TaskInspector task={task} />
    }

    if (item.type === 'mutation') {
      const mutation = mutations.get(item.id)
      if (!mutation) return <EmptyInspector />
      return <MutationInspector mutation={mutation} />
    }

    if (item.type === 'terminal') {
      const [actor, taskId] = item.id.split('::')
      if (!actor || !taskId) return <EmptyInspector />
      return <TerminalInspector taskId={taskId} actor={actor} />
    }

    return <EmptyInspector />
  })()

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-3 py-2 border-b">
        <span className="text-xs font-medium capitalize">{item.type} Inspector</span>
        <Button variant="ghost" size="icon" className="size-6" onClick={onClose}>
          <X className="size-3" />
        </Button>
      </div>
      <ScrollArea className="flex-1">
        <div className="p-3">{content}</div>
      </ScrollArea>
    </div>
  )
}
