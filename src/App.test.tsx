import { render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'

const invokeMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

describe('App', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_tasks') {
        return []
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

      return null
    })
  })

  it('renders the task creation view', async () => {
    render(<App />)

    expect(
      screen.getByRole('heading', { name: /Autonomous Orchestration Platform/i }),
    ).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Create Task/i })).toBeInTheDocument()

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('get_tasks')
    })
  })
})
