import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'

import TaskBudgetPanel from '@/components/TaskBudgetPanel'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { controlExecutionScope, getMissionControlSnapshot, getTasks, listTerminalEvents } from '@/hooks/useTauri'
import { useAopStore } from '@/store/aop-store'
import type {
  AgentEventRecord,
  AgentRunRecord,
  ExecutionScopeType,
  MissionControlSnapshot,
  TaskControlAction,
  TaskRecord,
  TerminalEventRecord,
} from '@/types'

type DensityMode = 'pro' | 'balanced' | 'minimal'

interface MissionFilters {
  tier: 'all' | '1' | '2' | '3'
  status: 'all' | 'pending' | 'executing' | 'paused' | 'completed' | 'failed'
  provider: string
  modelId: string
  persona: string
  skill: string
  mcpServer: string
  mcpTool: string
  timeWindow: '5m' | '15m' | '1h' | '6h' | '24h'
}

const DEFAULT_FILTERS: MissionFilters = {
  tier: 'all',
  status: 'all',
  provider: '',
  modelId: '',
  persona: '',
  skill: '',
  mcpServer: '',
  mcpTool: '',
  timeWindow: '1h',
}

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'short',
    timeStyle: 'medium',
  }).format(new Date(timestamp * 1000))
}

function statusVariant(status: string | null): 'default' | 'secondary' | 'destructive' | 'outline' {
  if (status === 'executing') {
    return 'default'
  }
  if (status === 'pending' || status === 'paused') {
    return 'secondary'
  }
  if (status === 'failed') {
    return 'destructive'
  }
  return 'outline'
}

function includesFilter(value: string | null, query: string): boolean {
  if (!query.trim()) {
    return true
  }
  return value?.toLowerCase().includes(query.trim().toLowerCase()) ?? false
}

function timeWindowSeconds(window: MissionFilters['timeWindow']): number {
  switch (window) {
    case '5m':
      return 5 * 60
    case '15m':
      return 15 * 60
    case '1h':
      return 60 * 60
    case '6h':
      return 6 * 60 * 60
    case '24h':
    default:
      return 24 * 60 * 60
  }
}

function eventBelongsToRun(event: AgentEventRecord, run: AgentRunRecord): boolean {
  if (event.runId && event.runId === run.id) {
    return true
  }
  return event.actor === run.actor && event.taskId != null && run.taskId != null && event.taskId === run.taskId
}

function terminalEventMessage(event: TerminalEventRecord): string {
  if (event.details?.trim()) {
    return event.details
  }
  return event.action
}

export function MissionControlView() {
  const tasksMap = useAopStore((state) => state.tasks)
  const addTask = useAopStore((state) => state.addTask)
  const selectTask = useAopStore((state) => state.selectTask)

  const tasks = useMemo(
    () => Array.from(tasksMap.values()).sort((left, right) => right.createdAt - left.createdAt),
    [tasksMap],
  )
  const taskById = useMemo(() => {
    const map = new Map<string, TaskRecord>()
    tasks.forEach((task) => map.set(task.id, task))
    return map
  }, [tasks])

  const rootByTaskId = useMemo(() => {
    const map = new Map<string, string>()
    tasks.forEach((task) => {
      let current: TaskRecord | undefined = task
      let safety = 0
      while (current?.parentId && safety < 48) {
        current = taskById.get(current.parentId)
        safety += 1
      }
      if (current?.id) {
        map.set(task.id, current.id)
      }
    })
    return map
  }, [taskById, tasks])

  const rootTasks = useMemo(
    () =>
      tasks
        .filter((task) => task.tier === 1 && task.parentId === null)
        .sort((left, right) => right.createdAt - left.createdAt),
    [tasks],
  )

  const [snapshot, setSnapshot] = useState<MissionControlSnapshot | null>(null)
  const [densityMode, setDensityMode] = useState<DensityMode>('balanced')
  const [filters, setFilters] = useState<MissionFilters>(DEFAULT_FILTERS)
  const [selectedRootTaskId, setSelectedRootTaskId] = useState<string>('')
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [scopeType, setScopeType] = useState<ExecutionScopeType>('tree')
  const [scopeTier, setScopeTier] = useState<'1' | '2' | '3'>('2')

  const [isLoadingSnapshot, setIsLoadingSnapshot] = useState(false)
  const [snapshotError, setSnapshotError] = useState<string | null>(null)
  const [scopeError, setScopeError] = useState<string | null>(null)
  const [activeControlAction, setActiveControlAction] = useState<TaskControlAction | null>(null)

  const [terminalEvents, setTerminalEvents] = useState<TerminalEventRecord[]>([])
  const [isLoadingTerminal, setIsLoadingTerminal] = useState(false)
  const [terminalError, setTerminalError] = useState<string | null>(null)

  useEffect(() => {
    if (selectedRootTaskId && rootTasks.some((task) => task.id === selectedRootTaskId)) {
      return
    }
    setSelectedRootTaskId(rootTasks[0]?.id ?? '')
  }, [rootTasks, selectedRootTaskId])

  const loadTasks = useCallback(async () => {
    try {
      const fetched = await getTasks()
      fetched.forEach((task) => addTask(task))
    } catch {
      // Mission Control should stay usable even if task sync fails.
    }
  }, [addTask])

  const loadSnapshot = useCallback(async () => {
    setIsLoadingSnapshot(true)
    try {
      const input = selectedRootTaskId
        ? { rootTaskId: selectedRootTaskId, limit: 300 }
        : ({ limit: 300 } as const)
      const next = await getMissionControlSnapshot(input)
      setSnapshot(next)
      setSnapshotError(null)
    } catch (error) {
      setSnapshotError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoadingSnapshot(false)
    }
  }, [selectedRootTaskId])

  useEffect(() => {
    void loadTasks()
  }, [loadTasks])

  useEffect(() => {
    void loadSnapshot()
    const intervalRef = setInterval(() => {
      void loadSnapshot()
    }, 1600)
    return () => clearInterval(intervalRef)
  }, [loadSnapshot])

  const filteredEvents = useMemo(() => {
    const now = Math.floor(Date.now() / 1000)
    const cutoff = now - timeWindowSeconds(filters.timeWindow)
    return (snapshot?.recentEvents ?? []).filter((event) => {
      if (selectedRootTaskId && event.rootTaskId !== selectedRootTaskId) {
        return false
      }
      if (event.createdAt < cutoff) {
        return false
      }
      if (filters.tier !== 'all' && Number(filters.tier) !== event.tier) {
        return false
      }
      if (filters.status !== 'all' && event.status !== filters.status) {
        return false
      }
      if (!includesFilter(event.provider, filters.provider)) {
        return false
      }
      if (!includesFilter(event.modelId, filters.modelId)) {
        return false
      }
      if (!includesFilter(event.persona, filters.persona)) {
        return false
      }
      if (!includesFilter(event.skill, filters.skill)) {
        return false
      }
      if (!includesFilter(event.mcpServer, filters.mcpServer)) {
        return false
      }
      if (!includesFilter(event.mcpTool, filters.mcpTool)) {
        return false
      }
      return true
    })
  }, [filters, selectedRootTaskId, snapshot?.recentEvents])

  const filteredRuns = useMemo(() => {
    const requiresMcpFilter = Boolean(filters.mcpServer.trim() || filters.mcpTool.trim())
    return (snapshot?.activeRuns ?? []).filter((run) => {
      if (selectedRootTaskId && run.rootTaskId !== selectedRootTaskId) {
        return false
      }
      if (filters.tier !== 'all' && Number(filters.tier) !== run.tier) {
        return false
      }
      if (filters.status !== 'all' && run.status !== filters.status) {
        return false
      }
      if (!includesFilter(run.provider, filters.provider)) {
        return false
      }
      if (!includesFilter(run.modelId, filters.modelId)) {
        return false
      }
      if (!includesFilter(run.persona, filters.persona)) {
        return false
      }
      if (!includesFilter(run.skill, filters.skill)) {
        return false
      }
      if (requiresMcpFilter) {
        const hasMcpEvent = filteredEvents.some((event) => eventBelongsToRun(event, run))
        if (!hasMcpEvent) {
          return false
        }
      }
      return true
    })
  }, [
    filteredEvents,
    filters.mcpServer,
    filters.mcpTool,
    filters.modelId,
    filters.persona,
    filters.provider,
    filters.skill,
    filters.status,
    filters.tier,
    selectedRootTaskId,
    snapshot?.activeRuns,
  ])

  useEffect(() => {
    if (selectedRunId && filteredRuns.some((run) => run.id === selectedRunId)) {
      return
    }
    setSelectedRunId(filteredRuns[0]?.id ?? null)
  }, [filteredRuns, selectedRunId])

  const selectedRun = useMemo(
    () => filteredRuns.find((run) => run.id === selectedRunId) ?? null,
    [filteredRuns, selectedRunId],
  )

  const selectedRunEvents = useMemo(() => {
    if (!selectedRun) {
      return filteredEvents.slice(0, 150)
    }
    return filteredEvents.filter((event) => eventBelongsToRun(event, selectedRun)).slice(0, 250)
  }, [filteredEvents, selectedRun])

  const tasksInSelectedRoot = useMemo(() => {
    if (!selectedRootTaskId) {
      return [] as TaskRecord[]
    }
    return tasks.filter((task) => rootByTaskId.get(task.id) === selectedRootTaskId)
  }, [rootByTaskId, selectedRootTaskId, tasks])

  const scopeTaskForBudget = useMemo(() => {
    if (!selectedRootTaskId) {
      return null
    }
    if (scopeType === 'tree') {
      return taskById.get(selectedRootTaskId) ?? null
    }
    if (scopeType === 'agent') {
      return selectedRun?.taskId ? (taskById.get(selectedRun.taskId) ?? null) : null
    }
    const targetTier = Number(scopeTier)
    const selectedTask = selectedRun?.taskId ? taskById.get(selectedRun.taskId) ?? null : null
    if (selectedTask && selectedTask.tier === targetTier) {
      return selectedTask
    }
    return tasksInSelectedRoot.find((task) => task.tier === targetTier) ?? null
  }, [scopeTier, scopeType, selectedRootTaskId, selectedRun?.taskId, taskById, tasksInSelectedRoot])

  const loadTerminal = useCallback(async () => {
    if (!selectedRun?.taskId) {
      setTerminalEvents([])
      setTerminalError(null)
      return
    }
    setIsLoadingTerminal(true)
    try {
      const events = await listTerminalEvents({
        actor: selectedRun.actor,
        taskId: selectedRun.taskId,
        limit: 300,
      })
      setTerminalEvents(events)
      setTerminalError(null)
    } catch (error) {
      setTerminalError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsLoadingTerminal(false)
    }
  }, [selectedRun?.actor, selectedRun?.taskId])

  useEffect(() => {
    void loadTerminal()
    const intervalRef = setInterval(() => {
      void loadTerminal()
    }, 1400)
    return () => clearInterval(intervalRef)
  }, [loadTerminal])

  const runWithMcpSummary = useMemo(() => {
    const byRun = new Map<string, { server: string | null; tool: string | null }>()
    filteredRuns.forEach((run) => byRun.set(run.id, { server: null, tool: null }))
    filteredEvents.forEach((event) => {
      if (!event.mcpServer && !event.mcpTool) {
        return
      }
      const run = filteredRuns.find((candidate) => eventBelongsToRun(event, candidate))
      if (!run) {
        return
      }
      byRun.set(run.id, {
        server: event.mcpServer ?? null,
        tool: event.mcpTool ?? null,
      })
    })
    return byRun
  }, [filteredEvents, filteredRuns])

  async function handleControl(action: TaskControlAction) {
    if (!selectedRootTaskId) {
      return
    }
    setScopeError(null)
    setActiveControlAction(action)

    try {
      const updated = await controlExecutionScope({
        rootTaskId: selectedRootTaskId,
        action,
        scopeType,
        tier: scopeType === 'tier' ? Number(scopeTier) : undefined,
        agentTaskId: scopeType === 'agent' ? selectedRun?.taskId ?? undefined : undefined,
        reason: `mission control ${scopeType} ${action}`,
      })
      updated.forEach((task) => addTask(task))
      toast.success(`Scope ${scopeType} action '${action}' applied to ${updated.length} task(s).`)
      await loadSnapshot()
      await loadTasks()
    } catch (error) {
      setScopeError(error instanceof Error ? error.message : String(error))
    } finally {
      setActiveControlAction(null)
    }
  }

  const mosaicGridClass =
    densityMode === 'pro'
      ? 'grid grid-cols-1 gap-2 2xl:grid-cols-3 xl:grid-cols-2'
      : densityMode === 'minimal'
        ? 'grid grid-cols-1 gap-3'
        : 'grid grid-cols-1 gap-3 2xl:grid-cols-2'

  return (
    <div className="space-y-4">
      <Card className="border-primary/30 bg-gradient-to-r from-primary/5 via-background to-emerald-500/5">
        <CardHeader className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
          <div className="space-y-1">
            <CardTitle>Mission Control</CardTitle>
            <p className="text-muted-foreground text-sm">
              Real-time orchestration telemetry with smart control scopes and budget operations.
            </p>
          </div>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
            <div className="space-y-1">
              <Label htmlFor="mc-root">Root Task</Label>
              <Select onValueChange={setSelectedRootTaskId} value={selectedRootTaskId || '__empty__'}>
                <SelectTrigger id="mc-root" className="w-[260px] max-w-full">
                  <SelectValue placeholder="Select orchestration root" />
                </SelectTrigger>
                <SelectContent>
                  {rootTasks.length === 0 ? <SelectItem value="__empty__">No Tier 1 tasks</SelectItem> : null}
                  {rootTasks.map((task) => (
                    <SelectItem key={task.id} value={task.id}>
                      {task.id.slice(0, 8)} · {task.domain} · {task.status}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <Label htmlFor="mc-density">Density</Label>
              <Select onValueChange={(value) => setDensityMode(value as DensityMode)} value={densityMode}>
                <SelectTrigger id="mc-density" className="w-[170px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="pro">PRO</SelectItem>
                  <SelectItem value="balanced">Balanced</SelectItem>
                  <SelectItem value="minimal">Minimal</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-end">
              <Button onClick={() => void loadSnapshot()} size="sm" type="button" variant="outline">
                {isLoadingSnapshot ? 'Syncing...' : 'Refresh'}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-4 lg:grid-cols-8">
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Active runs</p>
            <p className="text-lg font-semibold">{filteredRuns.length}</p>
          </div>
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Events</p>
            <p className="text-lg font-semibold">{filteredEvents.length}</p>
          </div>
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Providers</p>
            <p className="text-lg font-semibold">{new Set(filteredRuns.map((run) => run.provider).filter(Boolean)).size}</p>
          </div>
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Models</p>
            <p className="text-lg font-semibold">{new Set(filteredRuns.map((run) => run.modelId).filter(Boolean)).size}</p>
          </div>
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Tokens In</p>
            <p className="text-lg font-semibold">
              {filteredRuns.reduce((sum, run) => sum + run.tokensIn, 0)}
            </p>
          </div>
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Tokens Out</p>
            <p className="text-lg font-semibold">
              {filteredRuns.reduce((sum, run) => sum + run.tokensOut, 0)}
            </p>
          </div>
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Delta</p>
            <p className="text-lg font-semibold">
              {filteredRuns.reduce((sum, run) => sum + run.tokenDelta, 0)}
            </p>
          </div>
          <div className="rounded-md border p-2">
            <p className="text-muted-foreground">Updated</p>
            <p className="text-sm font-semibold">{snapshot ? formatTimestamp(snapshot.generatedAt) : '-'}</p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Filters</CardTitle>
        </CardHeader>
        <CardContent className="grid grid-cols-1 gap-3 md:grid-cols-3 xl:grid-cols-6">
          <div className="space-y-1">
            <Label>Tier</Label>
            <Select onValueChange={(value) => setFilters((current) => ({ ...current, tier: value as MissionFilters['tier'] }))} value={filters.tier}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All</SelectItem>
                <SelectItem value="1">Tier 1</SelectItem>
                <SelectItem value="2">Tier 2</SelectItem>
                <SelectItem value="3">Tier 3</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1">
            <Label>Status</Label>
            <Select onValueChange={(value) => setFilters((current) => ({ ...current, status: value as MissionFilters['status'] }))} value={filters.status}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All</SelectItem>
                <SelectItem value="pending">Pending</SelectItem>
                <SelectItem value="executing">Executing</SelectItem>
                <SelectItem value="paused">Paused</SelectItem>
                <SelectItem value="completed">Completed</SelectItem>
                <SelectItem value="failed">Failed</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1">
            <Label>Time</Label>
            <Select onValueChange={(value) => setFilters((current) => ({ ...current, timeWindow: value as MissionFilters['timeWindow'] }))} value={filters.timeWindow}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="5m">5m</SelectItem>
                <SelectItem value="15m">15m</SelectItem>
                <SelectItem value="1h">1h</SelectItem>
                <SelectItem value="6h">6h</SelectItem>
                <SelectItem value="24h">24h</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1">
            <Label>Provider</Label>
            <Input
              onChange={(event) => setFilters((current) => ({ ...current, provider: event.target.value }))}
              placeholder="claude_code, openai"
              value={filters.provider}
            />
          </div>

          <div className="space-y-1">
            <Label>Model / Persona / Skill</Label>
            <div className="space-y-2">
              <Input
                onChange={(event) => setFilters((current) => ({ ...current, modelId: event.target.value }))}
                placeholder="model id"
                value={filters.modelId}
              />
              <Input
                onChange={(event) => setFilters((current) => ({ ...current, persona: event.target.value }))}
                placeholder="persona"
                value={filters.persona}
              />
              <Input
                onChange={(event) => setFilters((current) => ({ ...current, skill: event.target.value }))}
                placeholder="skill"
                value={filters.skill}
              />
            </div>
          </div>

          <div className="space-y-1">
            <Label>MCP Server / Tool</Label>
            <div className="space-y-2">
              <Input
                onChange={(event) => setFilters((current) => ({ ...current, mcpServer: event.target.value }))}
                placeholder="filesystem"
                value={filters.mcpServer}
              />
              <Input
                onChange={(event) => setFilters((current) => ({ ...current, mcpTool: event.target.value }))}
                placeholder="read_file"
                value={filters.mcpTool}
              />
            </div>
          </div>
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-[1.1fr_1fr]">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle>Agent Mosaic</CardTitle>
            <Badge variant="outline">{densityMode.toUpperCase()}</Badge>
          </CardHeader>
          <CardContent>
            {snapshotError ? <p className="text-destructive mb-3 text-sm whitespace-pre-wrap">{snapshotError}</p> : null}
            <div className={mosaicGridClass}>
              {filteredRuns.map((run) => {
                const runMcp = runWithMcpSummary.get(run.id)
                const isSelected = selectedRun?.id === run.id
                return (
                  <button
                    className={`rounded-md border p-3 text-left transition-colors ${
                      isSelected ? 'border-primary bg-accent/30' : 'hover:bg-muted/30'
                    }`}
                    key={run.id}
                    onClick={() => {
                      setSelectedRunId(run.id)
                      if (run.taskId) {
                        selectTask(run.taskId)
                      }
                    }}
                    type="button"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <strong className="text-xs">{run.actor}</strong>
                      <Badge variant={statusVariant(run.status)}>{run.status}</Badge>
                    </div>
                    <p className="text-muted-foreground text-[11px]">tier {run.tier ?? '-'} · task {run.taskId?.slice(0, 8) ?? 'global'}</p>
                    <p className="text-sm">{run.provider ?? 'n/a'} / {run.modelId ?? 'n/a'}</p>
                    <p className="text-muted-foreground text-[11px]">
                      persona {run.persona ?? '-'} · skill {run.skill ?? '-'}
                    </p>
                    <p className="text-muted-foreground text-[11px]">
                      MCP {runMcp?.server ?? '-'} / {runMcp?.tool ?? '-'}
                    </p>
                    <p className="mt-1 text-[11px]">
                      in {run.tokensIn} · out {run.tokensOut} · delta {run.tokenDelta}
                    </p>
                  </button>
                )
              })}
            </div>
            {filteredRuns.length === 0 ? (
              <p className="text-muted-foreground mt-3 text-sm">No active runs match current filters.</p>
            ) : null}
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Execution Controls</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                <div className="space-y-1">
                  <Label>Scope</Label>
                  <Select onValueChange={(value) => setScopeType(value as ExecutionScopeType)} value={scopeType}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="tree">Tree</SelectItem>
                      <SelectItem value="tier">Tier</SelectItem>
                      <SelectItem value="agent">Agent</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1">
                  <Label>Tier</Label>
                  <Select onValueChange={(value) => setScopeTier(value as '1' | '2' | '3')} value={scopeTier}>
                    <SelectTrigger disabled={scopeType !== 'tier'}>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="1">Tier 1</SelectItem>
                      <SelectItem value="2">Tier 2</SelectItem>
                      <SelectItem value="3">Tier 3</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1">
                  <Label>Agent</Label>
                  <Input
                    disabled
                    value={scopeType === 'agent' ? selectedRun?.taskId ?? 'Select an agent run' : 'Scope not agent'}
                  />
                </div>
              </div>

              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={activeControlAction !== null || !selectedRootTaskId}
                  onClick={() => void handleControl('pause')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeControlAction === 'pause' ? 'Pausing...' : 'Pause'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !selectedRootTaskId}
                  onClick={() => void handleControl('resume')}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeControlAction === 'resume' ? 'Resuming...' : 'Resume'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !selectedRootTaskId}
                  onClick={() => void handleControl('stop')}
                  size="sm"
                  type="button"
                  variant="destructive"
                >
                  {activeControlAction === 'stop' ? 'Stopping...' : 'Stop'}
                </Button>
                <Button
                  disabled={activeControlAction !== null || !selectedRootTaskId}
                  onClick={() => void handleControl('restart')}
                  size="sm"
                  type="button"
                  variant="secondary"
                >
                  {activeControlAction === 'restart' ? 'Restarting...' : 'Restart'}
                </Button>
              </div>

              {scopeError ? <p className="text-destructive text-xs whitespace-pre-wrap">{scopeError}</p> : null}
            </CardContent>
          </Card>

          <TaskBudgetPanel
            includeDescendants={scopeType === 'tree'}
            onChanged={async () => {
              await loadTasks()
              await loadSnapshot()
            }}
            task={scopeTaskForBudget}
            title={`Budget · ${scopeType.toUpperCase()} scope`}
          />

          <Card>
            <CardHeader>
              <CardTitle>
                Agent Detail
                {selectedRun ? ` · ${selectedRun.actor} · ${selectedRun.taskId?.slice(0, 8) ?? 'global'}` : ''}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <Tabs defaultValue="timeline">
                <TabsList>
                  <TabsTrigger value="timeline">Timeline</TabsTrigger>
                  <TabsTrigger value="terminal">Terminal</TabsTrigger>
                </TabsList>
                <TabsContent value="timeline">
                  <ScrollArea className="h-[360px] rounded-md border p-2">
                    <div className="space-y-2 text-xs">
                      {selectedRunEvents.map((event) => (
                        <div className="rounded-md border p-2" key={event.id}>
                          <div className="flex items-center justify-between gap-2">
                            <strong>{event.action}</strong>
                            <Badge variant={statusVariant(event.status)}>{event.status ?? 'n/a'}</Badge>
                          </div>
                          <p className="text-muted-foreground">{formatTimestamp(event.createdAt)}</p>
                          <p className="text-muted-foreground">
                            {event.provider ?? '-'} / {event.modelId ?? '-'} · {event.persona ?? '-'} · {event.skill ?? '-'}
                          </p>
                          <p>
                            MCP {event.mcpServer ?? '-'} / {event.mcpTool ?? '-'} · latency {event.latencyMs ?? 0}ms · retry {event.retryCount ?? 0}
                          </p>
                          {event.message ? <p className="mt-1 whitespace-pre-wrap">{event.message}</p> : null}
                          {event.payloadJson ? (
                            <pre className="bg-muted mt-1 overflow-hidden rounded p-2 text-[11px] whitespace-pre-wrap">
                              {event.payloadJson}
                            </pre>
                          ) : null}
                        </div>
                      ))}
                      {selectedRunEvents.length === 0 ? (
                        <p className="text-muted-foreground">No timeline events for the selected scope.</p>
                      ) : null}
                    </div>
                  </ScrollArea>
                </TabsContent>
                <TabsContent value="terminal">
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <Badge variant="outline">{terminalEvents.length} lines</Badge>
                      <Button onClick={() => void loadTerminal()} size="sm" type="button" variant="outline">
                        {isLoadingTerminal ? 'Syncing...' : 'Refresh'}
                      </Button>
                    </div>
                    {terminalError ? <p className="text-destructive text-xs whitespace-pre-wrap">{terminalError}</p> : null}
                    <ScrollArea className="h-[320px] rounded-md border bg-muted/20 p-2">
                      <div className="space-y-1 font-mono text-xs">
                        {terminalEvents
                          .slice()
                          .reverse()
                          .map((event) => (
                            <div className="rounded px-2 py-1" key={event.id}>
                              <span className="text-muted-foreground">[{formatTimestamp(event.timestamp)}]</span>{' '}
                              <span>{terminalEventMessage(event)}</span>
                            </div>
                          ))}
                        {terminalEvents.length === 0 ? (
                          <p className="text-muted-foreground">No terminal activity for the selected agent.</p>
                        ) : null}
                      </div>
                    </ScrollArea>
                  </div>
                </TabsContent>
              </Tabs>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}
