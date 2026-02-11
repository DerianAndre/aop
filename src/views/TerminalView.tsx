import { useCallback, useEffect, useMemo, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { listAgentTerminals, listTerminalEvents } from '@/hooks/useTauri'
import { useAopStore } from '@/store/aop-store'
import type { AgentTerminalSession, TerminalEventRecord } from '@/types'

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'short',
    timeStyle: 'medium',
  }).format(new Date(timestamp * 1000))
}

function sessionStatusVariant(status: string | null): 'default' | 'secondary' | 'destructive' | 'outline' {
  if (status === 'executing') {
    return 'default'
  }
  if (status === 'paused' || status === 'pending') {
    return 'secondary'
  }
  if (status === 'failed') {
    return 'destructive'
  }
  return 'outline'
}

function terminalKey(actor: string, taskId: string): string {
  return `${actor}::${taskId}`
}

export function TerminalView() {
  const tasksMap = useAopStore((state) => state.tasks)
  const selectedTaskId = useAopStore((state) => state.selectedTaskId)
  const tasks = useMemo(() => Array.from(tasksMap.values()), [tasksMap])
  const tier1Tasks = useMemo(
    () =>
      tasks
        .filter((task) => task.tier === 1)
        .sort((left, right) => right.createdAt - left.createdAt),
    [tasks],
  )

  const defaultRootTaskId = useMemo(() => {
    if (selectedTaskId) {
      const selectedTask = tasks.find((task) => task.id === selectedTaskId)
      if (selectedTask?.tier === 1) {
        return selectedTask.id
      }
      if (selectedTask?.parentId) {
        const parentTask = tasks.find((task) => task.id === selectedTask.parentId)
        if (parentTask?.tier === 1) {
          return parentTask.id
        }
      }
    }

    const executingRoot = tier1Tasks.find((task) => task.status === 'executing')
    return executingRoot?.id ?? tier1Tasks[0]?.id ?? ''
  }, [selectedTaskId, tasks, tier1Tasks])

  const [rootTaskId, setRootTaskId] = useState(defaultRootTaskId || '__all__')
  const [includeInactive, setIncludeInactive] = useState(false)
  const [sessions, setSessions] = useState<AgentTerminalSession[]>([])
  const [events, setEvents] = useState<TerminalEventRecord[]>([])
  const [selectedTerminalKey, setSelectedTerminalKey] = useState<string | null>(null)
  const [isLoadingSessions, setIsLoadingSessions] = useState(false)
  const [isLoadingEvents, setIsLoadingEvents] = useState(false)
  const [sessionError, setSessionError] = useState<string | null>(null)
  const [eventsError, setEventsError] = useState<string | null>(null)

  useEffect(() => {
    if ((rootTaskId === '__all__' || !rootTaskId) && defaultRootTaskId) {
      setRootTaskId(defaultRootTaskId)
    }
  }, [defaultRootTaskId, rootTaskId])

  const selectedSession = useMemo(
    () =>
      selectedTerminalKey
        ? sessions.find(
            (session) => terminalKey(session.actor, session.taskId) === selectedTerminalKey,
          ) ?? null
        : null,
    [selectedTerminalKey, sessions],
  )

  const loadSessions = useCallback(async () => {
    setIsLoadingSessions(true)
    try {
      const nextSessions = await listAgentTerminals({
        rootTaskId: rootTaskId === '__all__' ? undefined : rootTaskId,
        includeDescendants: true,
        includeInactive,
        limit: 80,
      })

      setSessions(nextSessions)
      setSessionError(null)

      if (nextSessions.length === 0) {
        setSelectedTerminalKey(null)
        setEvents([])
        return
      }

      const previousKey = selectedTerminalKey
      const hasPrevious = previousKey
        ? nextSessions.some(
            (session) => terminalKey(session.actor, session.taskId) === previousKey,
          )
        : false

      if (!hasPrevious) {
        const nextDefaultKey = terminalKey(nextSessions[0].actor, nextSessions[0].taskId)
        setSelectedTerminalKey(nextDefaultKey)
      }
    } catch (error) {
      setSessionError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoadingSessions(false)
    }
  }, [includeInactive, rootTaskId, selectedTerminalKey])

  const loadEvents = useCallback(async () => {
    if (!selectedSession) {
      setEvents([])
      return
    }

    setIsLoadingEvents(true)
    try {
      const nextEvents = await listTerminalEvents({
        actor: selectedSession.actor,
        taskId: selectedSession.taskId,
        limit: 300,
      })
      setEvents(nextEvents)
      setEventsError(null)
    } catch (error) {
      setEventsError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoadingEvents(false)
    }
  }, [selectedSession])

  useEffect(() => {
    void loadSessions()
    const intervalRef = setInterval(() => {
      void loadSessions()
    }, 1800)
    return () => clearInterval(intervalRef)
  }, [loadSessions])

  useEffect(() => {
    void loadEvents()
    const intervalRef = setInterval(() => {
      void loadEvents()
    }, 1200)
    return () => clearInterval(intervalRef)
  }, [loadEvents])

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[340px_1fr]">
      <Card>
        <CardHeader className="space-y-3">
          <CardTitle>Agent Terminals</CardTitle>
          <div className="space-y-2">
            <Label htmlFor="terminal-root-task">Root Task</Label>
            <Select onValueChange={setRootTaskId} value={rootTaskId}>
              <SelectTrigger className="w-full" id="terminal-root-task">
                <SelectValue placeholder="All active terminals" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="__all__">All root tasks</SelectItem>
                {tier1Tasks.map((task) => (
                  <SelectItem key={task.id} value={task.id}>
                    {task.id.slice(0, 8)} · {task.domain} · {task.status}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex gap-2">
            <Button
              onClick={() => setIncludeInactive((value) => !value)}
              size="sm"
              type="button"
              variant={includeInactive ? 'default' : 'outline'}
            >
              {includeInactive ? 'Including Inactive' : 'Only Active'}
            </Button>
            <Button onClick={() => void loadSessions()} size="sm" type="button" variant="outline">
              {isLoadingSessions ? 'Syncing...' : 'Refresh'}
            </Button>
          </div>
          {sessionError ? <p className="text-destructive text-xs whitespace-pre-wrap">{sessionError}</p> : null}
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-[620px]">
            <div className="space-y-2">
              {sessions.map((session) => {
                const key = terminalKey(session.actor, session.taskId)
                const isSelected = selectedTerminalKey === key
                return (
                  <button
                    className={`w-full rounded-md border p-3 text-left ${
                      isSelected ? 'border-primary bg-accent/30' : ''
                    }`}
                    key={key}
                    onClick={() => setSelectedTerminalKey(key)}
                    type="button"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <strong className="text-xs">{session.actor}</strong>
                      <Badge variant={sessionStatusVariant(session.taskStatus)}>{session.taskStatus ?? 'unknown'}</Badge>
                    </div>
                    <p className="text-muted-foreground text-[11px]">
                      task {session.taskId.slice(0, 8)} · tier {session.taskTier ?? '-'} · {session.taskDomain ?? '-'}
                    </p>
                    <p className="text-muted-foreground text-[11px]">events {session.eventCount}</p>
                    <p className="mt-1 text-[11px]">{session.lastAction}</p>
                  </button>
                )
              })}
              {sessions.length === 0 ? (
                <p className="text-muted-foreground text-sm">
                  No active agent terminals found.
                </p>
              ) : null}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>
            Terminal Output
            {selectedSession ? ` · ${selectedSession.actor} · ${selectedSession.taskId.slice(0, 8)}` : ''}
          </CardTitle>
          <Button onClick={() => void loadEvents()} size="sm" type="button" variant="outline">
            {isLoadingEvents ? 'Syncing...' : 'Refresh'}
          </Button>
        </CardHeader>
        <CardContent className="space-y-3">
          {eventsError ? <p className="text-destructive text-xs whitespace-pre-wrap">{eventsError}</p> : null}
          <div className="rounded-md border bg-muted/25 p-2">
            <ScrollArea className="h-[620px]">
              <div className="space-y-1 font-mono text-xs">
                {events
                  .slice()
                  .reverse()
                  .map((event) => (
                    <div className="rounded px-2 py-1" key={event.id}>
                      <span className="text-muted-foreground">[{formatTimestamp(event.timestamp)}]</span>{' '}
                      <span className="text-primary">{event.action}</span>{' '}
                      {event.details ? <span>{event.details}</span> : null}
                    </div>
                  ))}
                {!selectedSession ? (
                  <p className="text-muted-foreground">Select an agent terminal to view output.</p>
                ) : null}
                {selectedSession && events.length === 0 ? (
                  <p className="text-muted-foreground">No output yet for this terminal.</p>
                ) : null}
              </div>
            </ScrollArea>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
