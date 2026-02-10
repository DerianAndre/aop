import { useCallback, useEffect, useMemo, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { listTaskBudgetRequests, requestTaskBudgetIncrease, resolveTaskBudgetRequest } from '@/hooks/useTauri'
import type { BudgetRequestRecord, TaskRecord } from '@/types'

interface TaskBudgetPanelProps {
  task: TaskRecord | null
  title?: string
  includeDescendants?: boolean
  onChanged?: () => Promise<void> | void
}

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000))
}

function requestStatusVariant(status: string): 'default' | 'secondary' | 'destructive' | 'outline' {
  if (status === 'approved') {
    return 'default'
  }
  if (status === 'rejected') {
    return 'destructive'
  }
  if (status === 'pending') {
    return 'secondary'
  }
  return 'outline'
}

export default function TaskBudgetPanel({
  task,
  title = 'Token Budget Control',
  includeDescendants = false,
  onChanged,
}: TaskBudgetPanelProps) {
  const [increment, setIncrement] = useState(500)
  const [reason, setReason] = useState('')
  const [requests, setRequests] = useState<BudgetRequestRecord[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [activeAction, setActiveAction] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [feedback, setFeedback] = useState<string | null>(null)

  const pendingRequests = useMemo(
    () => requests.filter((request) => request.status === 'pending'),
    [requests],
  )

  const loadRequests = useCallback(async () => {
    if (!task) {
      setRequests([])
      return
    }

    setIsLoading(true)
    try {
      const result = await listTaskBudgetRequests({
        taskId: task.id,
        includeDescendants,
        limit: 20,
      })
      setRequests(result)
      setError(null)
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError))
    } finally {
      setIsLoading(false)
    }
  }, [includeDescendants, task])

  useEffect(() => {
    void loadRequests()
  }, [loadRequests])

  async function handleIncreaseNow() {
    if (!task) {
      return
    }
    if (!Number.isFinite(increment) || increment <= 0) {
      setError('Increment must be greater than 0.')
      return
    }

    setActiveAction('increase_now')
    setFeedback(null)
    setError(null)
    try {
      await requestTaskBudgetIncrease({
        taskId: task.id,
        requestedBy: 'ui',
        reason: reason.trim() || 'manual on-demand token budget increase',
        requestedIncrement: Math.floor(increment),
        autoApprove: true,
      })
      await loadRequests()
      if (onChanged) {
        await onChanged()
      }
      setFeedback(`Budget increased by ${Math.floor(increment)} tokens.`)
      setReason('')
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : String(requestError))
    } finally {
      setActiveAction(null)
    }
  }

  async function handleCreateRequest() {
    if (!task) {
      return
    }
    if (!Number.isFinite(increment) || increment <= 0) {
      setError('Increment must be greater than 0.')
      return
    }

    setActiveAction('request_approval')
    setFeedback(null)
    setError(null)
    try {
      await requestTaskBudgetIncrease({
        taskId: task.id,
        requestedBy: 'ui',
        reason: reason.trim() || 'manual approval request for token budget increase',
        requestedIncrement: Math.floor(increment),
        autoApprove: false,
      })
      await loadRequests()
      setFeedback(`Budget request created for +${Math.floor(increment)} tokens.`)
      setReason('')
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : String(requestError))
    } finally {
      setActiveAction(null)
    }
  }

  async function handleResolve(request: BudgetRequestRecord, decision: 'approve' | 'reject') {
    setActiveAction(`${decision}:${request.id}`)
    setFeedback(null)
    setError(null)
    try {
      await resolveTaskBudgetRequest({
        requestId: request.id,
        decision,
        approvedIncrement: decision === 'approve' ? request.requestedIncrement : undefined,
        reason:
          decision === 'approve'
            ? 'approved from UI budget panel'
            : 'rejected from UI budget panel',
        decidedBy: 'ui',
        resumeTask: decision === 'approve',
      })
      await loadRequests()
      if (onChanged) {
        await onChanged()
      }
      setFeedback(
        decision === 'approve'
          ? `Approved request ${request.id.slice(0, 8)}.`
          : `Rejected request ${request.id.slice(0, 8)}.`,
      )
    } catch (resolveError) {
      setError(resolveError instanceof Error ? resolveError.message : String(resolveError))
    } finally {
      setActiveAction(null)
    }
  }

  if (!task) {
    return null
  }

  const remaining = task.tokenBudget - task.tokenUsage
  const usageRatio = task.tokenBudget > 0 ? (task.tokenUsage / task.tokenBudget) * 100 : 0

  return (
    <div className="space-y-3 rounded-md border p-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-sm font-semibold">{title}</p>
        <div className="flex items-center gap-2">
          <Badge variant="outline">{task.tokenUsage}/{task.tokenBudget}</Badge>
          <Badge variant={usageRatio >= 85 ? 'destructive' : 'secondary'}>
            {usageRatio.toFixed(1)}%
          </Badge>
          {isLoading ? <Badge variant="outline">syncing</Badge> : null}
        </div>
      </div>

      <p className="text-muted-foreground text-xs">Remaining: {remaining} tokens</p>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-[180px_1fr]">
        <div className="space-y-1">
          <Label htmlFor={`budget-increment-${task.id}`}>Increment</Label>
          <Input
            id={`budget-increment-${task.id}`}
            min={1}
            onChange={(event) => setIncrement(Number(event.target.value || 0))}
            step={50}
            type="number"
            value={increment}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor={`budget-reason-${task.id}`}>Reason</Label>
          <Input
            id={`budget-reason-${task.id}`}
            onChange={(event) => setReason(event.target.value)}
            placeholder="Why this task needs more tokens"
            value={reason}
          />
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          disabled={activeAction !== null}
          onClick={() => void handleIncreaseNow()}
          size="sm"
          type="button"
        >
          {activeAction === 'increase_now' ? 'Increasing...' : 'Increase Now'}
        </Button>
        <Button
          disabled={activeAction !== null}
          onClick={() => void handleCreateRequest()}
          size="sm"
          type="button"
          variant="outline"
        >
          {activeAction === 'request_approval' ? 'Requesting...' : 'Request Approval'}
        </Button>
      </div>

      {pendingRequests.length > 0 ? (
        <div className="space-y-2">
          <p className="text-sm font-medium">Pending Requests</p>
          {pendingRequests.map((request) => (
            <div className="rounded-md border p-2" key={request.id}>
              <div className="flex flex-wrap items-center justify-between gap-2">
                <p className="text-xs">
                  +{request.requestedIncrement} by {request.requestedBy}
                </p>
                <div className="flex gap-2">
                  <Button
                    disabled={activeAction !== null}
                    onClick={() => void handleResolve(request, 'approve')}
                    size="sm"
                    type="button"
                    variant="outline"
                  >
                    {activeAction === `approve:${request.id}` ? 'Approving...' : 'Approve'}
                  </Button>
                  <Button
                    disabled={activeAction !== null}
                    onClick={() => void handleResolve(request, 'reject')}
                    size="sm"
                    type="button"
                    variant="destructive"
                  >
                    {activeAction === `reject:${request.id}` ? 'Rejecting...' : 'Reject'}
                  </Button>
                </div>
              </div>
              <p className="text-muted-foreground text-[11px]">{request.reason}</p>
            </div>
          ))}
        </div>
      ) : null}

      {requests.length > 0 ? (
        <div className="space-y-2">
          <p className="text-sm font-medium">Recent Requests</p>
          <div className="space-y-1">
            {requests.slice(0, 6).map((request) => (
              <div className="rounded-md border p-2" key={request.id}>
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="text-xs">+{request.requestedIncrement}</span>
                  <Badge variant={requestStatusVariant(request.status)}>{request.status}</Badge>
                </div>
                <p className="text-muted-foreground text-[11px]">
                  {request.reason}
                </p>
                <p className="text-muted-foreground text-[11px]">
                  {formatTimestamp(request.createdAt)}
                </p>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {feedback ? <p className="text-xs">{feedback}</p> : null}
      {error ? <p className="text-destructive text-xs whitespace-pre-wrap">{error}</p> : null}
    </div>
  )
}
