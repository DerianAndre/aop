import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'

const invokeMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

describe('App', () => {
  beforeEach(() => {
    window.localStorage.setItem(
      'aop-storage',
      JSON.stringify({
        state: {
          activeTab: 'tasks',
          taskFilter: {},
          targetProject: '',
          mcpCommand: '',
          mcpArgs: '',
        },
        version: 0,
      }),
    )

    invokeMock.mockReset()
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_tasks') {
        return []
      }

      if (command === 'get_mission_control_snapshot') {
        return {
          generatedAt: 1_718_234_567,
          activeRuns: [],
          recentEvents: [],
          modelHealth: [],
        }
      }

      if (command === 'create_task') {
        return {
          id: '1',
          parentId: null,
          tier: 1,
          domain: 'platform',
          objective: 'Bootstrap',
          status: 'pending',
          tokenBudget: 3000,
          tokenUsage: 0,
          contextEfficiencyRatio: 0,
          riskFactor: 0,
          complianceScore: 0,
          checksumBefore: null,
          checksumAfter: null,
          errorMessage: null,
          retryCount: 0,
          createdAt: 1_718_234_567,
          updatedAt: 1_718_234_567,
        }
      }

      if (command === 'get_default_target_project') {
        return 'C:/tmp/project'
      }

      if (command === 'list_task_activity') {
        return []
      }

      if (command === 'list_task_budget_requests') {
        return []
      }

      if (command === 'list_agent_terminals') {
        return []
      }

      if (command === 'list_terminal_events') {
        return []
      }

      return null
    })
  })

  it('opens task dialog and creates a task', async () => {
    render(<App />)

    fireEvent.click(screen.getByRole('button', { name: /^Tasks$/i }))

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /Task Hierarchy/i })).toBeInTheDocument()
    })

    fireEvent.click(screen.getByRole('button', { name: /New Task/i }))
    expect(screen.getByRole('dialog', { name: /Create Task/i })).toBeInTheDocument()

    fireEvent.change(screen.getByLabelText(/Objective/i), {
      target: { value: 'Bootstrap foundation' },
    })
    fireEvent.click(screen.getByRole('button', { name: /^Create Task$/i }))

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('get_tasks')
    })

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('create_task', {
        input: {
          parentId: null,
          tier: 1,
          domain: 'platform',
          objective: 'Bootstrap foundation',
          tokenBudget: 3000,
        },
      })
    })
  })
})
