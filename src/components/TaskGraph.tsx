import { useMemo } from 'react'

import { Background, Controls, MiniMap, ReactFlow } from '@xyflow/react'
import type { Edge, Node, NodeMouseHandler } from '@xyflow/react'

import type { TaskRecord } from '@/types'

interface TaskGraphProps {
  tasks: TaskRecord[]
  selectedTaskId: string | null
  onTaskClick: (taskId: string) => void
  onTaskDoubleClick: (taskId: string) => void
}

const STATUS_BG: Record<string, string> = {
  pending: '#e5e7eb',
  executing: '#bfdbfe',
  completed: '#bbf7d0',
  failed: '#fecaca',
  paused: '#fef08a',
}

const STATUS_BORDER: Record<string, string> = {
  pending: '#6b7280',
  executing: '#2563eb',
  completed: '#16a34a',
  failed: '#dc2626',
  paused: '#ca8a04',
}

function buildNodes(tasks: TaskRecord[], selectedTaskId: string | null): Node[] {
  const sorted = [...tasks].sort((left, right) => {
    if (left.tier !== right.tier) return left.tier - right.tier
    if (left.createdAt !== right.createdAt) return left.createdAt - right.createdAt
    return left.id.localeCompare(right.id)
  })

  const indexByTier = new Map<number, number>()

  return sorted.map((task) => {
    const tier = Math.max(1, task.tier)
    const rowIndex = indexByTier.get(tier) ?? 0
    indexByTier.set(tier, rowIndex + 1)

    const isSelected = task.id === selectedTaskId
    return {
      id: task.id,
      position: {
        x: (tier - 1) * 360,
        y: rowIndex * 150,
      },
      data: {
        label: (
          <div>
            <strong>{task.domain}</strong>
            <div style={{ fontSize: '0.78rem', marginTop: '0.2rem' }}>{task.objective.slice(0, 58)}</div>
            <div style={{ fontSize: '0.72rem', marginTop: '0.25rem' }}>
              Tier {task.tier} | {task.status} | tokens {task.tokenUsage}/{task.tokenBudget}
            </div>
          </div>
        ),
      },
      style: {
        width: 290,
        borderRadius: 12,
        border: `2px solid ${isSelected ? '#111827' : STATUS_BORDER[task.status] ?? '#6b7280'}`,
        background: STATUS_BG[task.status] ?? '#f3f4f6',
        padding: 8,
      },
    }
  })
}

function buildEdges(tasks: TaskRecord[]): Edge[] {
  return tasks
    .filter((task) => task.parentId)
    .map((task) => ({
      id: `${task.parentId}-${task.id}`,
      source: task.parentId as string,
      target: task.id,
      animated: task.status === 'executing',
      style: { stroke: '#6b7280', strokeWidth: 1.5 },
    }))
}

function TaskGraph({ tasks, selectedTaskId, onTaskClick, onTaskDoubleClick }: TaskGraphProps) {
  const nodes = useMemo(() => buildNodes(tasks, selectedTaskId), [tasks, selectedTaskId])
  const edges = useMemo(() => buildEdges(tasks), [tasks])
  const handleNodeClick: NodeMouseHandler = (_event, node) => {
    onTaskClick(node.id)
  }
  const handleNodeDoubleClick: NodeMouseHandler = (_event, node) => {
    onTaskDoubleClick(node.id)
  }

  if (tasks.length === 0) {
    return <p className="empty-state">No tasks available to render the graph.</p>
  }

  return (
    <div className="task-graph">
      <ReactFlow
        fitView
        edges={edges}
        nodes={nodes}
        onNodeClick={handleNodeClick}
        onNodeDoubleClick={handleNodeDoubleClick}
      >
        <MiniMap />
        <Controls />
        <Background />
      </ReactFlow>
    </div>
  )
}

export default TaskGraph
