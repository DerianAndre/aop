import { useEffect, useRef, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { listTaskActivity } from '@/hooks/useTauri'
import type { AuditLogEntry } from '@/types'
import { toast } from 'sonner'

interface TaskActivityFeedProps {
  taskId: string | null
  title?: string
  includeDescendants?: boolean
  limit?: number
  pollMs?: number
  enableBudgetToasts?: boolean
}

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000))
}

export default function TaskActivityFeed({
  taskId,
  title = 'Live Activity',
  includeDescendants = true,
  limit = 120,
  pollMs = 1500,
  enableBudgetToasts = true,
}: TaskActivityFeedProps) {
  const [entries, setEntries] = useState<AuditLogEntry[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const initializedRef = useRef(false)
  const lastSeenIdRef = useRef(0)

  useEffect(() => {
    initializedRef.current = false
    lastSeenIdRef.current = 0
  }, [taskId])

  useEffect(() => {
    if (!taskId) {
      setEntries([])
      setError(null)
      setIsLoading(false)
      return
    }

    let isCancelled = false
    let intervalRef: ReturnType<typeof setInterval> | null = null

    const fetchActivity = async () => {
      try {
        setIsLoading(true)
        const nextEntries = await listTaskActivity({
          taskId,
          includeDescendants,
          limit,
        })
        if (!isCancelled) {
          if (enableBudgetToasts && nextEntries.length > 0) {
            const maxId = nextEntries.reduce((max, entry) => Math.max(max, entry.id), 0)
            if (!initializedRef.current) {
              initializedRef.current = true
              lastSeenIdRef.current = maxId
            } else {
              const unseenEntries = nextEntries
                .filter((entry) => entry.id > lastSeenIdRef.current)
                .sort((left, right) => left.id - right.id)

              unseenEntries.forEach((entry) => {
                if (entry.action === 'token_budget_increase_requested') {
                  toast.info(`Budget request created (${entry.targetId?.slice(0, 8) ?? 'task'})`, {
                    description: entry.details ?? undefined,
                  })
                } else if (
                  entry.action === 'token_budget_auto_increase_applied' ||
                  entry.action === 'task_budget_auto_approved'
                ) {
                  toast.success(`Budget increased (${entry.targetId?.slice(0, 8) ?? 'task'})`, {
                    description: entry.details ?? undefined,
                  })
                } else if (entry.action === 'task_budget_request_resolved') {
                  if (entry.details?.includes('"status":"rejected"')) {
                    toast.warning(`Budget request rejected (${entry.targetId?.slice(0, 8) ?? 'task'})`, {
                      description: entry.details ?? undefined,
                    })
                  } else {
                    toast.success(`Budget request resolved (${entry.targetId?.slice(0, 8) ?? 'task'})`, {
                      description: entry.details ?? undefined,
                    })
                  }
                }
              })
              lastSeenIdRef.current = Math.max(lastSeenIdRef.current, maxId)
            }
          }
          setEntries(nextEntries)
          setError(null)
        }
      } catch (loadError) {
        if (!isCancelled) {
          setError(loadError instanceof Error ? loadError.message : String(loadError))
        }
      } finally {
        if (!isCancelled) {
          setIsLoading(false)
        }
      }
    }

    void fetchActivity()
    intervalRef = setInterval(() => {
      void fetchActivity()
    }, pollMs)

    return () => {
      isCancelled = true
      if (intervalRef) {
        clearInterval(intervalRef)
      }
    }
  }, [enableBudgetToasts, includeDescendants, limit, pollMs, taskId])

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        <p className="text-sm font-semibold">{title}</p>
        <div className="flex items-center gap-2">
          <Badge variant="outline">{entries.length} events</Badge>
          {isLoading ? <Badge variant="secondary">syncing</Badge> : null}
        </div>
      </div>

      {error ? <p className="text-destructive text-xs whitespace-pre-wrap">{error}</p> : null}

      <ScrollArea className="h-[260px] rounded-md border p-2">
        <div className="space-y-2">
          {entries.map((entry) => (
            <div className="rounded-md border p-2" key={entry.id}>
              <div className="flex items-center justify-between gap-2">
                <strong className="text-xs">{entry.action}</strong>
                <span className="text-muted-foreground text-[11px]">{formatTimestamp(entry.timestamp)}</span>
              </div>
              <p className="text-muted-foreground text-[11px]">actor: {entry.actor}</p>
              {entry.targetId ? <p className="text-muted-foreground text-[11px]">task: {entry.targetId}</p> : null}
              {entry.details ? <p className="mt-1 text-[11px] whitespace-pre-wrap">{entry.details}</p> : null}
            </div>
          ))}

          {!taskId ? (
            <p className="text-muted-foreground text-sm">Select a task to monitor orchestrator/agent activity.</p>
          ) : null}
          {taskId && entries.length === 0 && !isLoading ? (
            <p className="text-muted-foreground text-sm">No activity yet for this task.</p>
          ) : null}
        </div>
      </ScrollArea>
    </div>
  )
}
