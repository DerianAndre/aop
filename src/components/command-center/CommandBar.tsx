import { useCallback, useEffect, useRef, useState } from 'react'

import {
  Loader2,
  Play,
  CheckCircle2,
  GitPullRequest,
  Settings2,
} from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Separator } from '@/components/ui/separator'
import { useAopStore } from '@/store/aop-store'
import type { CcPhase } from '@/store/types'
import type { TaskRecord } from '@/types'

interface CommandBarProps {
  phase: CcPhase
  tasks: TaskRecord[]
  pendingMutationCount: number
  pendingBudgetCount: number
  errorCount: number
  isLoading: boolean
  onDecompose: (objective: string, budget: number, risk: number) => void
  onApprove: () => void
}

const ACTION_CONFIG: Record<CcPhase, { label: string; icon: typeof Play; variant: 'default' | 'secondary' | 'outline' | 'destructive' }> = {
  empty: { label: 'Decompose', icon: Play, variant: 'default' },
  planning: { label: 'Planning...', icon: Loader2, variant: 'secondary' },
  ready: { label: 'Approve & Execute', icon: CheckCircle2, variant: 'default' },
  running: { label: 'Running...', icon: Loader2, variant: 'secondary' },
  review: { label: 'Review Mutations', icon: GitPullRequest, variant: 'default' },
  completed: { label: 'New Objective', icon: Play, variant: 'outline' },
  failed: { label: 'Retry', icon: Play, variant: 'destructive' },
}

export default function CommandBar({
  phase,
  tasks,
  pendingMutationCount,
  pendingBudgetCount,
  errorCount,
  isLoading,
  onDecompose,
  onApprove,
}: CommandBarProps) {
  const { targetProject, mcpCommand, mcpArgs, setTargetProject, setMcpCommand, setMcpArgs } = useAopStore()

  const [objective, setObjective] = useState('')
  const [budget, setBudget] = useState(5000)
  const [risk, setRisk] = useState(0.5)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const activeTasks = tasks.filter((t) => t.status === 'executing').length
  const totalTokens = tasks.reduce((sum, t) => sum + t.tokenUsage, 0)

  const actionConfig = ACTION_CONFIG[phase]
  const ActionIcon = actionConfig.icon
  const isActionDisabled = isLoading || phase === 'planning' || phase === 'running' || (phase === 'empty' && !objective.trim())

  const handleAction = useCallback(() => {
    if (phase === 'empty' || phase === 'completed' || phase === 'failed') {
      onDecompose(objective, budget, risk)
    } else if (phase === 'ready') {
      onApprove()
    }
  }, [phase, objective, budget, risk, onDecompose, onApprove])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault()
        if (!isActionDisabled) handleAction()
      }
    },
    [handleAction, isActionDisabled]
  )

  useEffect(() => {
    const textarea = textareaRef.current
    if (textarea) {
      textarea.style.height = 'auto'
      textarea.style.height = `${Math.min(textarea.scrollHeight, 120)}px`
    }
  }, [objective])

  return (
    <div className="flex items-start gap-3 border-b bg-background/80 backdrop-blur-sm px-4 py-3">
      {/* Objective Input */}
      <div className="flex-1 min-w-0">
        <textarea
          ref={textareaRef}
          value={objective}
          onChange={(e) => setObjective(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Describe your objective..."
          disabled={phase === 'planning' || phase === 'running'}
          rows={1}
          className="w-full resize-none bg-transparent text-sm leading-relaxed placeholder:text-muted-foreground/60 focus:outline-none disabled:opacity-50 min-h-[36px]"
        />
      </div>

      {/* Compact Controls */}
      <div className="flex items-center gap-2 shrink-0">
        {/* Stats Badges */}
        {activeTasks > 0 && (
          <Badge variant="secondary" className="text-xs tabular-nums">
            {activeTasks} active
          </Badge>
        )}
        {totalTokens > 0 && (
          <Badge variant="outline" className="text-xs tabular-nums">
            {totalTokens.toLocaleString()} tok
          </Badge>
        )}
        {pendingMutationCount > 0 && (
          <Badge variant="default" className="text-xs tabular-nums">
            {pendingMutationCount} review
          </Badge>
        )}
        {pendingBudgetCount > 0 && (
          <Badge className="bg-warning text-warning-foreground text-xs tabular-nums">
            {pendingBudgetCount} budget
          </Badge>
        )}
        {errorCount > 0 && (
          <Badge variant="destructive" className="text-xs tabular-nums">
            {errorCount} err
          </Badge>
        )}

        <Separator orientation="vertical" className="h-6" />

        {/* Settings Popover */}
        <Popover>
          <PopoverTrigger asChild>
            <Button variant="ghost" size="icon" className="size-8">
              <Settings2 className="size-4" />
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-72" align="end">
            <div className="space-y-3">
              <h4 className="text-sm font-medium">Configuration</h4>
              <div className="space-y-2">
                <Label className="text-xs">Target Project</Label>
                <Input
                  value={targetProject}
                  onChange={(e) => setTargetProject(e.target.value)}
                  placeholder="/path/to/project"
                  className="h-8 text-xs"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div className="space-y-1">
                  <Label className="text-xs">Token Budget</Label>
                  <Input
                    type="number"
                    value={budget}
                    onChange={(e) => setBudget(Number(e.target.value))}
                    className="h-8 text-xs"
                  />
                </div>
                <div className="space-y-1">
                  <Label className="text-xs">Risk Tolerance</Label>
                  <Input
                    type="number"
                    value={risk}
                    onChange={(e) => setRisk(Number(e.target.value))}
                    step={0.1}
                    min={0}
                    max={1}
                    className="h-8 text-xs"
                  />
                </div>
              </div>
              <div className="space-y-2">
                <Label className="text-xs">MCP Command</Label>
                <Input
                  value={mcpCommand}
                  onChange={(e) => setMcpCommand(e.target.value)}
                  placeholder="local"
                  className="h-8 text-xs"
                />
              </div>
              <div className="space-y-2">
                <Label className="text-xs">MCP Args</Label>
                <Input
                  value={mcpArgs}
                  onChange={(e) => setMcpArgs(e.target.value)}
                  placeholder="comma,separated,args"
                  className="h-8 text-xs"
                />
              </div>
            </div>
          </PopoverContent>
        </Popover>

        {/* Primary Action Button */}
        <Button
          onClick={handleAction}
          disabled={isActionDisabled}
          variant={actionConfig.variant}
          size="sm"
          className="gap-1.5"
        >
          <ActionIcon className={`size-3.5 ${phase === 'planning' || phase === 'running' ? 'animate-spin' : ''}`} />
          {actionConfig.label}
        </Button>
      </div>
    </div>
  )
}
