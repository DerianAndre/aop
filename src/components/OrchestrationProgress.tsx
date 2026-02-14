import { useEffect, useRef, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { listTaskActivity } from '@/hooks/useTauri'
import type { AuditLogEntry } from '@/types'

type FlowType = 'quick-decompose' | 'analyze' | 'generate-plan' | 'approve-execute'

interface OrchestrationProgressProps {
  taskId: string
  flow: FlowType
  pollMs?: number
}

interface StepDef {
  action: string
  label: string
  metaKey?: string
}

const FLOW_STEPS: Record<FlowType, StepDef[]> = {
  'quick-decompose': [
    { action: 'orchestration_started', label: 'Scanning project files', metaKey: 'files' },
    { action: 'orchestration_context_built', label: 'Context built', metaKey: 'candidateFiles' },
    { action: 'plan_assignment_created', label: 'Creating assignments' },
    { action: 'orchestration_plan_ready', label: 'Plan ready', metaKey: 'assignments' },
  ],
  analyze: [
    { action: 'objective_analysis_started', label: 'Scanning project files', metaKey: 'files' },
    { action: '_analysis_active', label: 'LLM analyzing...' },
    { action: 'objective_analysis_completed', label: 'Analysis complete', metaKey: 'questions' },
  ],
  'generate-plan': [
    { action: 'plan_generation_started', label: 'Processing with LLM', metaKey: 'model' },
    { action: 'plan_assignment_created', label: 'Creating assignments' },
    { action: 'plan_generation_completed', label: 'Plan generated', metaKey: 'assignments' },
  ],
  'approve-execute': [
    { action: 'orchestration_spawn_started', label: 'Spawning agents', metaKey: 'plannedAssignments' },
    { action: 'tier2_execution_started', label: 'Domain leader executing', metaKey: 'domain' },
    { action: 'specialist_execution_started', label: 'Specialist generating code', metaKey: 'model' },
    { action: 'specialist_proposal_persisted', label: 'Code proposal created', metaKey: 'file' },
    { action: 'orchestration_spawn_completed', label: 'Execution complete', metaKey: 'appliedMutations' },
  ],
}

const ACTION_LABELS: Record<string, string> = {
  orchestration_started: 'Orchestration started',
  orchestration_context_built: 'Context built',
  orchestration_plan_ready: 'Plan ready for review',
  orchestration_spawn_started: 'Spawning agents',
  orchestration_spawn_completed: 'Execution complete',
  objective_analysis_started: 'Analyzing objective',
  objective_analysis_completed: 'Analysis complete',
  plan_generation_started: 'Generating plan',
  plan_generation_completed: 'Plan generated',
  plan_assignment_created: 'Assignment created',
  assignment_execution_failed: 'Assignment failed',
  tier2_execution_started: 'Domain leader started',
  tier2_context_ready: 'Context ready',
  tier3_task_created: 'Specialist task created',
  specialist_execution_started: 'Specialist started',
  specialist_proposal_persisted: 'Proposal persisted',
  specialist_execution_failed: 'Specialist failed',
  tier2_review_gate: 'Review gate',
  tier3_planned_execution_started: 'Specialist executing',
  tier3_planned_execution_completed: 'Specialist completed',
  task_status_changed: 'Status changed',
  token_budget_increase_requested: 'Budget increase requested',
  token_budget_auto_increase_applied: 'Budget auto-increased',
  task_budget_auto_approved: 'Budget auto-approved',
  task_budget_request_resolved: 'Budget request resolved',
}

const FLOW_LABELS: Record<FlowType, string> = {
  'quick-decompose': 'Quick Decompose',
  analyze: 'Analyze Objective',
  'generate-plan': 'Generate Plan',
  'approve-execute': 'Execute Plan',
}

function getPhase(action: string): string {
  if (action.startsWith('objective_analysis') || action === 'orchestration_started') return 'Analysis'
  if (action.startsWith('plan_') || action.startsWith('orchestration_context')) return 'Planning'
  return 'Execution'
}

function parseDetails(details: string | null): Record<string, string> {
  if (!details) return {}
  const result: Record<string, string> = {}
  const regex = /(\w+)=([^\s,]+)/g
  let match: RegExpExecArray | null
  while ((match = regex.exec(details)) !== null) {
    result[match[1]] = match[2]
  }
  return result
}

function relativeTime(ts: number, now: number): string {
  const diff = Math.max(0, now - ts)
  if (diff < 60) return `${diff}s`
  return `${Math.floor(diff / 60)}m${String(diff % 60).padStart(2, '0')}s`
}

function formatElapsed(seconds: number): string {
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}

function truncateDetails(details: string): string {
  if (details.length <= 80) return details
  return details.slice(0, 77) + '...'
}

function tierBadgeClass(entry: AuditLogEntry): string {
  const actor = entry.actor.toLowerCase()
  if (actor.includes('tier1') || actor.includes('orchestrator') || actor === 't1') {
    return 'bg-blue-500/15 text-blue-700 dark:text-blue-400'
  }
  if (actor.includes('tier2') || actor.includes('domain_leader') || actor === 't2') {
    return 'bg-purple-500/15 text-purple-700 dark:text-purple-400'
  }
  if (actor.includes('tier3') || actor.includes('specialist') || actor === 't3') {
    return 'bg-green-500/15 text-green-700 dark:text-green-400'
  }
  return 'bg-muted text-muted-foreground'
}

function tierLabel(entry: AuditLogEntry): string {
  const actor = entry.actor.toLowerCase()
  if (actor.includes('tier1') || actor.includes('orchestrator') || actor === 't1') return 'T1'
  if (actor.includes('tier2') || actor.includes('domain_leader') || actor === 't2') return 'T2'
  if (actor.includes('tier3') || actor.includes('specialist') || actor === 't3') return 'T3'
  return 'SYS'
}

export default function OrchestrationProgress({
  taskId,
  flow,
  pollMs = 800,
}: OrchestrationProgressProps) {
  const [entries, setEntries] = useState<AuditLogEntry[]>([])
  const [elapsed, setElapsed] = useState(0)
  const startTimeRef = useRef(Date.now())
  const scrollRef = useRef<HTMLDivElement>(null)

  // Poll activity
  useEffect(() => {
    let isCancelled = false

    const fetchActivity = async () => {
      try {
        const nextEntries = await listTaskActivity({
          taskId,
          includeDescendants: true,
          limit: 80,
        })
        if (!isCancelled) {
          setEntries(nextEntries)
        }
      } catch {
        // Silently handle polling errors
      }
    }

    void fetchActivity()
    const interval = setInterval(() => void fetchActivity(), pollMs)

    return () => {
      isCancelled = true
      clearInterval(interval)
    }
  }, [taskId, pollMs])

  // Elapsed timer
  useEffect(() => {
    startTimeRef.current = Date.now()
    setElapsed(0)
    const interval = setInterval(() => {
      setElapsed(Math.floor((Date.now() - startTimeRef.current) / 1000))
    }, 1000)
    return () => clearInterval(interval)
  }, [taskId])

  // Auto-scroll log
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [entries.length])

  const seenActions = new Set(entries.map((e) => e.action))
  const steps = FLOW_STEPS[flow]

  // Compute step states
  type StepState = 'completed' | 'active' | 'pending'
  const stepStates: { def: StepDef; state: StepState; meta: string | null }[] = []
  let foundActive = false

  for (const def of steps) {
    if (def.action === '_analysis_active') {
      // Virtual step: active when analysis started but not completed
      if (seenActions.has('objective_analysis_started') && !seenActions.has('objective_analysis_completed')) {
        stepStates.push({ def, state: 'active', meta: null })
        foundActive = true
      } else if (seenActions.has('objective_analysis_completed')) {
        stepStates.push({ def, state: 'completed', meta: null })
      } else {
        stepStates.push({ def, state: 'pending', meta: null })
      }
      continue
    }

    const matchingEntries = entries.filter((e) => e.action === def.action)
    const isPresent = matchingEntries.length > 0

    let meta: string | null = null
    if (isPresent && def.metaKey) {
      const lastMatch = matchingEntries[matchingEntries.length - 1]
      const parsed = parseDetails(lastMatch.details)
      meta = parsed[def.metaKey] ?? null
    }
    if (isPresent && def.action === 'plan_assignment_created') {
      meta = `${matchingEntries.length}`
    }

    if (isPresent) {
      stepStates.push({ def, state: 'completed', meta })
    } else if (!foundActive) {
      const prevAllCompleted = stepStates.every((s) => s.state === 'completed')
      if (prevAllCompleted && stepStates.length > 0) {
        stepStates.push({ def, state: 'active', meta: null })
        foundActive = true
      } else {
        stepStates.push({ def, state: 'pending', meta: null })
      }
    } else {
      stepStates.push({ def, state: 'pending', meta: null })
    }
  }

  // Group entries by phase
  const phaseOrder = ['Analysis', 'Planning', 'Execution']
  const groupedEntries: { phase: string; items: AuditLogEntry[] }[] = []
  const phaseMap = new Map<string, AuditLogEntry[]>()

  const sortedEntries = [...entries].sort((a, b) => a.id - b.id)
  for (const entry of sortedEntries) {
    const phase = getPhase(entry.action)
    if (!phaseMap.has(phase)) {
      phaseMap.set(phase, [])
    }
    phaseMap.get(phase)!.push(entry)
  }
  for (const phase of phaseOrder) {
    const items = phaseMap.get(phase)
    if (items && items.length > 0) {
      groupedEntries.push({ phase, items })
    }
  }

  const nowEpoch = Math.floor(Date.now() / 1000)

  return (
    <div className="space-y-3 rounded-md border border-blue-500/20 bg-blue-500/5 p-4">
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <div className="size-2 animate-pulse rounded-full bg-blue-500" />
          <h4 className="text-sm font-semibold">{FLOW_LABELS[flow]}</h4>
        </div>
        <span className="font-mono text-xs text-muted-foreground">{formatElapsed(elapsed)}</span>
      </div>

      {/* Step checklist */}
      <div className="space-y-1.5">
        {stepStates.map(({ def, state, meta }, idx) => (
          <div className="flex items-center gap-2" key={idx}>
            {state === 'completed' ? (
              <svg className="size-4 shrink-0 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            ) : state === 'active' ? (
              <svg className="size-4 shrink-0 animate-spin text-blue-500" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
            ) : (
              <svg className="size-4 shrink-0 text-muted-foreground/40" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <circle cx="12" cy="12" r="9" />
              </svg>
            )}
            <span
              className={`text-xs ${
                state === 'completed'
                  ? 'text-foreground'
                  : state === 'active'
                    ? 'text-blue-600 dark:text-blue-400 font-medium'
                    : 'text-muted-foreground/50'
              }`}
            >
              {def.label}
            </span>
            {meta ? (
              <Badge variant="outline" className="ml-auto text-[10px] px-1.5 py-0">
                {meta}
              </Badge>
            ) : null}
          </div>
        ))}
      </div>

      {/* Grouped log */}
      {groupedEntries.length > 0 ? (
        <ScrollArea className="h-[200px] rounded-md border bg-background/50 p-2">
          <div ref={scrollRef} className="space-y-2">
            {groupedEntries.map(({ phase, items }) => (
              <div key={phase}>
                <div className="flex items-center gap-2 py-1">
                  <Separator className="flex-1" />
                  <span className="shrink-0 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    {phase}
                  </span>
                  <Separator className="flex-1" />
                </div>
                <div className="space-y-0.5">
                  {items.map((entry) => (
                    <div className="flex items-start gap-1.5 px-1 py-0.5" key={entry.id}>
                      <span
                        className={`mt-0.5 shrink-0 rounded px-1 py-0 text-[9px] font-bold leading-tight ${tierBadgeClass(entry)}`}
                      >
                        {tierLabel(entry)}
                      </span>
                      <span className="flex-1 text-[11px] text-foreground/80 leading-tight">
                        {ACTION_LABELS[entry.action] ?? entry.action.replace(/_/g, ' ')}
                        {entry.details ? (
                          <span className="text-muted-foreground ml-1">
                            {truncateDetails(entry.details)}
                          </span>
                        ) : null}
                      </span>
                      <span className="shrink-0 text-[10px] text-muted-foreground/60 tabular-nums">
                        {relativeTime(entry.timestamp, nowEpoch)}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </ScrollArea>
      ) : (
        <p className="text-muted-foreground text-xs">Waiting for activity...</p>
      )}
    </div>
  )
}
