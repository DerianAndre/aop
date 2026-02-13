import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import {
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  DollarSign,
  Terminal,
  Activity,
  Check,
  X,
} from 'lucide-react'

import TaskActivityFeed from '@/components/TaskActivityFeed'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  listAgentTerminals,
  listTerminalEvents,
  listTaskBudgetRequests,
  resolveTaskBudgetRequest,
} from '@/hooks/useTauri'
import type { CcLiveFeedTab } from '@/store/types'
import type {
  AgentTerminalSession,
  BudgetRequestRecord,
  TaskRecord,
  TerminalEventRecord,
} from '@/types'
import { toast } from 'sonner'

interface LiveFeedPanelProps {
  rootTaskId: string | null
  tasks: TaskRecord[]
  errorCount: number
  pendingBudgetCount: number
  isCollapsed: boolean
  activeTab: CcLiveFeedTab
  onTabChange: (tab: CcLiveFeedTab) => void
  onToggleCollapse: () => void
}

const TABS: { value: CcLiveFeedTab; label: string; icon: typeof Activity }[] = [
  { value: 'activity', label: 'Activity', icon: Activity },
  { value: 'terminal', label: 'Terminal', icon: Terminal },
  { value: 'budget', label: 'Budget', icon: DollarSign },
  { value: 'errors', label: 'Errors', icon: AlertTriangle },
]

function TerminalFeed({ rootTaskId }: { rootTaskId: string | null }) {
  const [sessions, setSessions] = useState<AgentTerminalSession[]>([])
  const [selectedSession, setSelectedSession] = useState<AgentTerminalSession | null>(null)
  const [events, setEvents] = useState<TerminalEventRecord[]>([])
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!rootTaskId) return
    let cancelled = false
    const poll = async () => {
      try {
        const result = await listAgentTerminals({
          rootTaskId,
          includeDescendants: true,
          includeInactive: true,
          limit: 50,
        })
        if (!cancelled) setSessions(result)
      } catch {
        /* silent */
      }
    }
    poll()
    const interval = setInterval(poll, 3000)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [rootTaskId])

  useEffect(() => {
    if (!selectedSession) return
    let cancelled = false
    const poll = async () => {
      try {
        const result = await listTerminalEvents({
          actor: selectedSession.actor,
          taskId: selectedSession.taskId,
          limit: 80,
        })
        if (!cancelled) {
          setEvents(result)
          if (scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight
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
  }, [selectedSession])

  if (sessions.length === 0) {
    return <p className="text-xs text-muted-foreground p-3">No agent terminals active.</p>
  }

  return (
    <div className="flex h-full">
      {/* Session List */}
      <div className="w-44 shrink-0 border-r overflow-y-auto">
        {sessions.map((s) => (
          <button
            key={`${s.actor}::${s.taskId}`}
            onClick={() => setSelectedSession(s)}
            className={`w-full text-left px-2 py-1.5 text-[11px] hover:bg-muted/50 transition-colors ${
              selectedSession?.actor === s.actor && selectedSession?.taskId === s.taskId
                ? 'bg-muted'
                : ''
            }`}
          >
            <div className="font-mono truncate">{s.actor}</div>
            <div className="text-muted-foreground truncate">{s.lastAction}</div>
          </button>
        ))}
      </div>
      {/* Event Stream */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-2 font-mono text-[11px] space-y-0.5">
        {selectedSession ? (
          events.map((ev) => (
            <div key={ev.id} className="flex gap-2">
              <span className="text-muted-foreground shrink-0 tabular-nums">
                {new Date(ev.timestamp * 1000).toLocaleTimeString()}
              </span>
              <span className="text-primary/70">{ev.action}</span>
              {ev.details && (
                <span className="text-muted-foreground truncate">{ev.details}</span>
              )}
            </div>
          ))
        ) : (
          <p className="text-muted-foreground p-2">Select a terminal session.</p>
        )}
      </div>
    </div>
  )
}

function BudgetFeed({ rootTaskId }: { rootTaskId: string | null }) {
  const [requests, setRequests] = useState<BudgetRequestRecord[]>([])
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set())

  useEffect(() => {
    if (!rootTaskId) return
    let cancelled = false
    const poll = async () => {
      try {
        const result = await listTaskBudgetRequests({
          taskId: rootTaskId,
          includeDescendants: true,
          limit: 50,
        })
        if (!cancelled) setRequests(result)
      } catch {
        /* silent */
      }
    }
    poll()
    const interval = setInterval(poll, 2000)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [rootTaskId])

  const handleResolve = useCallback(
    async (requestId: string, decision: 'approve' | 'reject') => {
      setBusyIds((prev) => new Set(prev).add(requestId))
      try {
        await resolveTaskBudgetRequest({
          requestId,
          decision,
          decidedBy: 'ui',
          resumeTask: decision === 'approve',
        })
        toast.success(`Budget request ${decision}d`)
      } catch (err) {
        toast.error(`Failed: ${err}`)
      } finally {
        setBusyIds((prev) => {
          const next = new Set(prev)
          next.delete(requestId)
          return next
        })
      }
    },
    []
  )

  const pending = requests.filter((r) => r.status === 'pending')
  const resolved = requests.filter((r) => r.status !== 'pending')

  return (
    <ScrollArea className="h-full">
      <div className="p-3 space-y-2">
        {pending.length > 0 && (
          <div className="space-y-1.5">
            <span className="text-[10px] font-medium uppercase text-muted-foreground">Pending</span>
            {pending.map((req) => (
              <div key={req.id} className="flex items-center gap-2 text-xs p-2 rounded-md bg-muted/30">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1">
                    <Badge variant="secondary" className="text-[10px]">{req.requestedBy}</Badge>
                    <span className="font-mono tabular-nums">+{req.requestedIncrement.toLocaleString()}</span>
                  </div>
                  <p className="text-muted-foreground truncate mt-0.5">{req.reason}</p>
                </div>
                <div className="flex gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-6"
                    disabled={busyIds.has(req.id)}
                    onClick={() => handleResolve(req.id, 'approve')}
                  >
                    <Check className="size-3 text-green-500" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-6"
                    disabled={busyIds.has(req.id)}
                    onClick={() => handleResolve(req.id, 'reject')}
                  >
                    <X className="size-3 text-red-500" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
        {resolved.length > 0 && (
          <div className="space-y-1">
            <span className="text-[10px] font-medium uppercase text-muted-foreground">Resolved</span>
            {resolved.slice(0, 20).map((req) => (
              <div key={req.id} className="flex items-center gap-2 text-[11px] text-muted-foreground py-1">
                <Badge
                  variant={req.status === 'approved' ? 'default' : 'destructive'}
                  className="text-[9px]"
                >
                  {req.status}
                </Badge>
                <span className="font-mono tabular-nums">+{req.requestedIncrement.toLocaleString()}</span>
                <span className="truncate">{req.reason}</span>
              </div>
            ))}
          </div>
        )}
        {requests.length === 0 && (
          <p className="text-xs text-muted-foreground text-center py-4">No budget requests.</p>
        )}
      </div>
    </ScrollArea>
  )
}

function ErrorsFeed({ tasks }: { tasks: TaskRecord[] }) {
  const failed = tasks.filter((t) => t.status === 'failed')

  return (
    <ScrollArea className="h-full">
      <div className="p-3 space-y-1.5">
        {failed.map((task) => (
          <div key={task.id} className="flex items-start gap-2 text-xs p-2 rounded-md bg-red-500/5">
            <AlertTriangle className="size-3.5 text-red-500 shrink-0 mt-0.5" />
            <div className="min-w-0">
              <div className="flex items-center gap-1.5">
                <Badge variant="outline" className="text-[10px]">T{task.tier}</Badge>
                <span className="truncate">{task.objective}</span>
              </div>
              {task.errorMessage && (
                <p className="text-red-500/80 font-mono text-[11px] mt-0.5 truncate">
                  {task.errorMessage}
                </p>
              )}
            </div>
          </div>
        ))}
        {failed.length === 0 && (
          <p className="text-xs text-muted-foreground text-center py-4">No errors.</p>
        )}
      </div>
    </ScrollArea>
  )
}

export default function LiveFeedPanel({
  rootTaskId,
  tasks,
  errorCount,
  pendingBudgetCount,
  isCollapsed,
  activeTab,
  onTabChange,
  onToggleCollapse,
}: LiveFeedPanelProps) {
  const badgeCounts: Partial<Record<CcLiveFeedTab, number>> = useMemo(
    () => ({
      budget: pendingBudgetCount,
      errors: errorCount,
    }),
    [pendingBudgetCount, errorCount]
  )

  return (
    <div className="flex flex-col h-full border-t bg-background">
      {/* Tab Bar */}
      <div className="flex items-center gap-0.5 px-2 py-1 border-b bg-muted/30">
        {TABS.map((tab) => {
          const Icon = tab.icon
          const count = badgeCounts[tab.value]
          return (
            <button
              key={tab.value}
              onClick={() => onTabChange(tab.value)}
              className={`flex items-center gap-1 px-2 py-1 rounded-sm text-[11px] transition-colors ${
                activeTab === tab.value
                  ? 'bg-background text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              <Icon className="size-3" />
              {tab.label}
              {count != null && count > 0 && (
                <Badge
                  variant={tab.value === 'errors' ? 'destructive' : 'secondary'}
                  className="text-[9px] px-1 py-0 h-3.5 min-w-[14px] justify-center"
                >
                  {count}
                </Badge>
              )}
            </button>
          )
        })}
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="icon"
          className="size-5"
          onClick={onToggleCollapse}
        >
          {isCollapsed ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />}
        </Button>
      </div>

      {/* Content */}
      {!isCollapsed && (
        <div className="flex-1 min-h-0">
          {activeTab === 'activity' && (
            <TaskActivityFeed
              taskId={rootTaskId}
              title=""
              includeDescendants
              pollMs={1500}
              enableBudgetToasts={false}
            />
          )}
          {activeTab === 'terminal' && <TerminalFeed rootTaskId={rootTaskId} />}
          {activeTab === 'budget' && <BudgetFeed rootTaskId={rootTaskId} />}
          {activeTab === 'errors' && <ErrorsFeed tasks={tasks} />}
        </div>
      )}
    </div>
  )
}
