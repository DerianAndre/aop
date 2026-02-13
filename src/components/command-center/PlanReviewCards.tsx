import { useState } from 'react'

import {
  ChevronDown,
  ChevronRight,
  FileCode,
  Shield,
  Zap,
} from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import type { OrchestrationResult, TaskAssignment } from '@/types'

interface PlanReviewCardsProps {
  result: OrchestrationResult
  onApprove: () => void
  onCancel: () => void
  isApproving: boolean
}

const DOMAIN_COLORS: Record<string, string> = {
  auth: 'bg-red-500/10 text-red-700 dark:text-red-400',
  database: 'bg-purple-500/10 text-purple-700 dark:text-purple-400',
  frontend: 'bg-blue-500/10 text-blue-700 dark:text-blue-400',
  api: 'bg-green-500/10 text-green-700 dark:text-green-400',
  platform: 'bg-orange-500/10 text-orange-700 dark:text-orange-400',
}

function AssignmentCard({ assignment }: { assignment: TaskAssignment }) {
  const [expanded, setExpanded] = useState(false)
  const domainClass = DOMAIN_COLORS[assignment.domain] ?? 'bg-muted text-muted-foreground'
  const riskPercent = Math.round(assignment.riskFactor * 100)

  return (
    <Card className="border-border/50">
      <CardHeader
        className="cursor-pointer py-3 px-4"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          {expanded ? <ChevronDown className="size-3.5 shrink-0" /> : <ChevronRight className="size-3.5 shrink-0" />}
          <Badge variant="outline" className={`text-[10px] ${domainClass}`}>
            {assignment.domain}
          </Badge>
          <Badge variant="secondary" className="text-[10px]">
            T{assignment.tier}
          </Badge>
          <span className="text-sm truncate flex-1">{assignment.objective}</span>
          <span className="text-xs text-muted-foreground tabular-nums shrink-0">
            {assignment.tokenBudget.toLocaleString()} tok
          </span>
        </div>
      </CardHeader>
      {expanded && (
        <CardContent className="pt-0 px-4 pb-3 space-y-2">
          <div className="flex items-center gap-4 text-xs text-muted-foreground">
            <span className="flex items-center gap-1">
              <Shield className="size-3" />
              Risk: {riskPercent}%
            </span>
            <span className="flex items-center gap-1">
              <Zap className="size-3" />
              Budget: {assignment.tokenBudget.toLocaleString()}
            </span>
          </div>
          {assignment.relevantFiles.length > 0 && (
            <div className="space-y-1">
              <span className="text-xs font-medium text-muted-foreground">Relevant files</span>
              <div className="flex flex-wrap gap-1">
                {assignment.relevantFiles.slice(0, 8).map((file) => (
                  <Badge key={file} variant="outline" className="text-[10px] font-mono">
                    <FileCode className="size-2.5 mr-1" />
                    {file.split('/').pop()}
                  </Badge>
                ))}
                {assignment.relevantFiles.length > 8 && (
                  <Badge variant="outline" className="text-[10px]">
                    +{assignment.relevantFiles.length - 8} more
                  </Badge>
                )}
              </div>
            </div>
          )}
          {assignment.constraints.length > 0 && (
            <div className="space-y-1">
              <span className="text-xs font-medium text-muted-foreground">Constraints</span>
              <ul className="text-xs text-muted-foreground space-y-0.5">
                {assignment.constraints.map((c, i) => (
                  <li key={i} className="pl-2 border-l-2 border-border">{c}</li>
                ))}
              </ul>
            </div>
          )}
        </CardContent>
      )}
    </Card>
  )
}

export default function PlanReviewCards({
  result,
  onApprove,
  onCancel,
  isApproving,
}: PlanReviewCardsProps) {
  const totalBudget = result.overheadBudget + result.distributedBudget + result.reserveBudget
  const overheadPct = Math.round((result.overheadBudget / totalBudget) * 100)
  const distributedPct = Math.round((result.distributedBudget / totalBudget) * 100)
  const reservePct = Math.round((result.reserveBudget / totalBudget) * 100)

  return (
    <div className="space-y-4 p-4">
      {/* Budget Overview */}
      <div className="space-y-2">
        <div className="flex items-center justify-between text-sm">
          <span className="font-medium">Budget Allocation</span>
          <span className="text-muted-foreground tabular-nums">
            {totalBudget.toLocaleString()} tokens total
          </span>
        </div>
        <div className="flex gap-0.5 h-2 rounded-full overflow-hidden">
          <div
            className="bg-muted-foreground/30 rounded-l-full"
            style={{ width: `${overheadPct}%` }}
            title={`Overhead: ${result.overheadBudget.toLocaleString()}`}
          />
          <div
            className="bg-primary"
            style={{ width: `${distributedPct}%` }}
            title={`Distributed: ${result.distributedBudget.toLocaleString()}`}
          />
          <div
            className="bg-muted-foreground/20 rounded-r-full"
            style={{ width: `${reservePct}%` }}
            title={`Reserve: ${result.reserveBudget.toLocaleString()}`}
          />
        </div>
        <div className="flex justify-between text-[10px] text-muted-foreground">
          <span>Overhead {overheadPct}%</span>
          <span>Distributed {distributedPct}%</span>
          <span>Reserve {reservePct}%</span>
        </div>
      </div>

      {/* Assignment Cards */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium">
            {result.assignments.length} Task{result.assignments.length !== 1 ? 's' : ''} Planned
          </span>
        </div>
        <div className="space-y-1.5">
          {result.assignments.map((assignment) => (
            <AssignmentCard key={assignment.taskId} assignment={assignment} />
          ))}
        </div>
      </div>

      {/* Actions */}
      <div className="flex justify-end gap-2 pt-2 border-t">
        <Button variant="outline" size="sm" onClick={onCancel} disabled={isApproving}>
          Cancel
        </Button>
        <Button size="sm" onClick={onApprove} disabled={isApproving}>
          {isApproving ? 'Approving...' : 'Approve & Execute'}
        </Button>
      </div>
    </div>
  )
}
