import { useCallback, useEffect, useState, type FormEvent } from 'react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { listAuditLog } from '@/hooks/useTauri'
import type { AuditLogEntry } from '@/types'

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000))
}

export function LogsView() {
  const [entries, setEntries] = useState<AuditLogEntry[]>([])
  const [targetId, setTargetId] = useState('')
  const [limit, setLimit] = useState(100)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const loadAuditLog = useCallback(async (overrideTargetId?: string) => {
    setIsLoading(true)
    setError(null)
    try {
      const nextEntries = await listAuditLog({
        targetId: (overrideTargetId ?? targetId).trim() || undefined,
        limit: Number.isFinite(limit) && limit > 0 ? Math.floor(limit) : 100,
      })
      setEntries(nextEntries)
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError))
    } finally {
      setIsLoading(false)
    }
  }, [limit, targetId])

  useEffect(() => {
    void loadAuditLog()
  }, [loadAuditLog])

  async function handleFilter(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    await loadAuditLog(targetId)
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>System Logs</CardTitle>
        <div className="flex gap-2">
          <Badge variant="outline">SQLite</Badge>
          <Badge variant="outline">Pipeline</Badge>
          <Badge variant="outline">MCP</Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <form className="grid grid-cols-1 gap-4 md:grid-cols-[1fr_160px_auto]" onSubmit={handleFilter}>
          <div className="space-y-2">
            <Label htmlFor="logs-target-id">Target ID (task/mutation optional)</Label>
            <Input
              id="logs-target-id"
              onChange={(event) => setTargetId(event.target.value)}
              placeholder="mutation-id or task-id"
              value={targetId}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="logs-limit">Limit</Label>
            <Input
              id="logs-limit"
              min={1}
              onChange={(event) => setLimit(Number(event.target.value || 100))}
              type="number"
              value={limit}
            />
          </div>
          <div className="flex items-end gap-2">
            <Button disabled={isLoading} type="submit">
              {isLoading ? 'Loading...' : 'Filter'}
            </Button>
            <Button disabled={isLoading} onClick={() => void loadAuditLog()} type="button" variant="outline">
              Refresh
            </Button>
          </div>
        </form>

        {error ? <p className="text-destructive text-sm">{error}</p> : null}

        <ScrollArea className="h-[560px]">
          <div className="space-y-2 font-mono text-xs">
            {entries.map((entry) => (
              <div className="rounded-md border p-3" key={entry.id}>
                <div className="flex items-center justify-between gap-2">
                  <strong>{entry.action}</strong>
                  <span className="text-muted-foreground">{formatTimestamp(entry.timestamp)}</span>
                </div>
                <p className="text-muted-foreground">actor: {entry.actor}</p>
                {entry.targetId ? <p className="text-muted-foreground">target: {entry.targetId}</p> : null}
                {entry.details ? <p className="mt-1 whitespace-pre-wrap">{entry.details}</p> : null}
              </div>
            ))}
            {entries.length === 0 ? <p className="text-muted-foreground">No logs found for current filter.</p> : null}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}
