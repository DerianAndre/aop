import { useEffect, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { listTaskActivity } from '@/hooks/useTauri'
import type { AuditLogEntry } from '@/types'

interface TaskActivityFeedProps {
  taskId: string | null
  title?: string
  includeDescendants?: boolean
  limit?: number
  pollMs?: number
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
}: TaskActivityFeedProps) {
  const [entries, setEntries] = useState<AuditLogEntry[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

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
  }, [includeDescendants, limit, pollMs, taskId])

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
