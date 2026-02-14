import { useMemo, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import type { GeneratedPlan, OrchestrationResult } from '@/types'

interface OrchestrationSummaryProps {
  result: OrchestrationResult
  generatedPlan?: GeneratedPlan | null
}

interface DomainSummary {
  domain: string
  taskCount: number
  totalBudget: number
  avgRisk: number
  files: string[]
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat().format(value)
}

function riskColor(risk: number): string {
  if (risk > 0.7) return 'text-red-600 dark:text-red-400'
  if (risk >= 0.3) return 'text-yellow-600 dark:text-yellow-400'
  return 'text-green-600 dark:text-green-400'
}

function riskBadgeVariant(risk: number): 'destructive' | 'secondary' | 'outline' {
  if (risk > 0.7) return 'destructive'
  if (risk >= 0.3) return 'secondary'
  return 'outline'
}

function riskLabel(risk: number): string {
  if (risk > 0.7) return 'High'
  if (risk >= 0.3) return 'Medium'
  return 'Low'
}

export default function OrchestrationSummary({
  result,
  generatedPlan,
}: OrchestrationSummaryProps) {
  const [filesExpanded, setFilesExpanded] = useState(false)

  const domainSummaries = useMemo<DomainSummary[]>(() => {
    const map = new Map<string, DomainSummary>()
    for (const assignment of result.assignments) {
      const existing = map.get(assignment.domain)
      if (existing) {
        existing.taskCount++
        existing.totalBudget += assignment.tokenBudget
        existing.avgRisk = (existing.avgRisk * (existing.taskCount - 1) + assignment.riskFactor) / existing.taskCount
        for (const file of assignment.relevantFiles) {
          if (!existing.files.includes(file)) {
            existing.files.push(file)
          }
        }
      } else {
        map.set(assignment.domain, {
          domain: assignment.domain,
          taskCount: 1,
          totalBudget: assignment.tokenBudget,
          avgRisk: assignment.riskFactor,
          files: [...assignment.relevantFiles],
        })
      }
    }
    return Array.from(map.values())
  }, [result.assignments])

  const totalFiles = useMemo(() => {
    const allFiles = new Set<string>()
    for (const assignment of result.assignments) {
      for (const file of assignment.relevantFiles) {
        allFiles.add(file)
      }
    }
    return allFiles
  }, [result.assignments])

  const maxRisk = useMemo(
    () => Math.max(0, ...result.assignments.map((a) => a.riskFactor)),
    [result.assignments],
  )

  const totalBudget = result.distributedBudget + result.overheadBudget + result.reserveBudget
  const distributedPct = totalBudget > 0 ? (result.distributedBudget / totalBudget) * 100 : 0
  const overheadPct = totalBudget > 0 ? (result.overheadBudget / totalBudget) * 100 : 0
  const reservePct = totalBudget > 0 ? (result.reserveBudget / totalBudget) * 100 : 0

  return (
    <div className="space-y-3 rounded-md border p-4">
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <h4 className="text-sm font-semibold">Orchestration Plan</h4>
        <div className="flex items-center gap-1.5">
          <Badge variant="outline" className="text-[10px]">
            {result.assignments.length} {result.assignments.length === 1 ? 'task' : 'tasks'}
          </Badge>
          <Badge variant="outline" className="text-[10px]">
            {totalFiles.size} {totalFiles.size === 1 ? 'file' : 'files'}
          </Badge>
          <Badge variant={riskBadgeVariant(maxRisk)} className="text-[10px]">
            {riskLabel(maxRisk)}
          </Badge>
        </div>
      </div>

      {/* Domain cards */}
      {domainSummaries.length > 0 ? (
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
          {domainSummaries.map((domain) => (
            <div
              className="rounded-md border bg-muted/30 p-3 space-y-1"
              key={domain.domain}
            >
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold capitalize">{domain.domain}</span>
                <Badge variant="outline" className="text-[9px] px-1 py-0">
                  T{result.assignments.find((a) => a.domain === domain.domain)?.tier ?? '?'}
                </Badge>
              </div>
              <p className="text-[11px] text-muted-foreground">
                {domain.taskCount} {domain.taskCount === 1 ? 'task' : 'tasks'} · {domain.files.length}{' '}
                {domain.files.length === 1 ? 'file' : 'files'}
              </p>
              <p className="text-[11px] text-muted-foreground">
                {formatNumber(domain.totalBudget)} tokens
              </p>
              <p className={`text-[11px] ${riskColor(domain.avgRisk)}`}>
                risk {(domain.avgRisk * 100).toFixed(0)}%
              </p>
            </div>
          ))}
        </div>
      ) : null}

      {/* Budget bar */}
      <div className="space-y-1.5">
        <div className="flex items-center justify-between">
          <span className="text-[11px] font-medium text-muted-foreground">Budget</span>
          <span className="text-[11px] text-muted-foreground">
            {formatNumber(totalBudget)} total
          </span>
        </div>
        <div className="flex h-2.5 w-full overflow-hidden rounded-full bg-muted">
          <div
            className="bg-blue-500 transition-all"
            style={{ width: `${distributedPct}%` }}
            title={`Tasks: ${formatNumber(result.distributedBudget)}`}
          />
          <div
            className="bg-yellow-500 transition-all"
            style={{ width: `${overheadPct}%` }}
            title={`Overhead: ${formatNumber(result.overheadBudget)}`}
          />
          <div
            className="bg-muted-foreground/30 transition-all"
            style={{ width: `${reservePct}%` }}
            title={`Reserve: ${formatNumber(result.reserveBudget)}`}
          />
        </div>
        <div className="flex gap-3 text-[10px] text-muted-foreground">
          <span className="flex items-center gap-1">
            <span className="inline-block size-2 rounded-full bg-blue-500" />
            Tasks ({formatNumber(result.distributedBudget)})
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block size-2 rounded-full bg-yellow-500" />
            Overhead ({formatNumber(result.overheadBudget)})
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block size-2 rounded-full bg-muted-foreground/30" />
            Reserve ({formatNumber(result.reserveBudget)})
          </span>
        </div>
      </div>

      {/* Risk assessment */}
      {generatedPlan?.riskAssessment ? (
        <div className="rounded-md border border-yellow-500/20 bg-yellow-500/5 p-2">
          <p className="text-[11px] text-foreground/80">{generatedPlan.riskAssessment}</p>
        </div>
      ) : null}

      {/* Files collapsible */}
      {totalFiles.size > 0 ? (
        <button
          className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
          onClick={() => setFilesExpanded((prev) => !prev)}
          type="button"
        >
          <span>{filesExpanded ? '\u25BE' : '\u25B8'}</span>
          <span>{totalFiles.size} targeted {totalFiles.size === 1 ? 'file' : 'files'}</span>
        </button>
      ) : null}
      {filesExpanded ? (
        <div className="rounded-md border bg-muted/30 p-2 space-y-0.5">
          {Array.from(totalFiles)
            .sort()
            .map((file) => (
              <p className="text-[11px] text-muted-foreground font-mono" key={file}>
                {file}
              </p>
            ))}
        </div>
      ) : null}
    </div>
  )
}
