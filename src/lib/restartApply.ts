import { executeDomainTask, listTaskMutations, runMutationPipeline } from '@/hooks/useTauri'
import type { ExecuteDomainTaskInput, MutationRecord, PipelineStepResult, TaskRecord } from '@/types'

const APPLYABLE_MUTATION_STATUSES = new Set(['proposed', 'validated', 'validated_no_tests'])
const DEFAULT_TOP_K = 8
const DEFAULT_MUTATION_POLL_ATTEMPTS = 8
const DEFAULT_MUTATION_POLL_INTERVAL_MS = 400

export interface RestartApplyInput {
  updatedTasks: TaskRecord[]
  targetProject: string
  mcpConfig?: Pick<ExecuteDomainTaskInput, 'mcpCommand' | 'mcpArgs'>
  topK?: number
  mutationPollAttempts?: number
  mutationPollIntervalMs?: number
}

export interface RestartApplySummary {
  tier2Tasks: number
  successfulTier2Tasks: number
  failedExecutions: number
  firstExecutionError: string | null
  tasksWithoutCandidates: number
  attemptedMutations: number
  appliedMutations: number
  rejectedMutations: number
  pipelineErrors: number
  firstRejectedReason: string | null
  firstPipelineError: string | null
}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => {
    setTimeout(resolve, ms)
  })

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function selectCandidateMutations(mutations: MutationRecord[]): MutationRecord[] {
  return mutations.filter((mutation) => APPLYABLE_MUTATION_STATUSES.has(mutation.status))
}

function getFirstFailedStep(steps: PipelineStepResult[]): PipelineStepResult | undefined {
  return steps.find((step) => step.status === 'failed')
}

async function waitForCandidateMutations(
  taskId: string,
  attempts: number,
  intervalMs: number,
): Promise<MutationRecord[]> {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const mutations = await listTaskMutations({ taskId })
    const candidates = selectCandidateMutations(mutations)
    if (candidates.length > 0) {
      return candidates
    }

    if (attempt < attempts - 1) {
      await sleep(intervalMs)
    }
  }

  return []
}

export async function executeRestartApply(input: RestartApplyInput): Promise<RestartApplySummary> {
  const tier2TaskIds = Array.from(new Set(input.updatedTasks.filter((task) => task.tier === 2).map((task) => task.id)))
  const attempts = Math.max(1, input.mutationPollAttempts ?? DEFAULT_MUTATION_POLL_ATTEMPTS)
  const intervalMs = Math.max(0, input.mutationPollIntervalMs ?? DEFAULT_MUTATION_POLL_INTERVAL_MS)

  if (tier2TaskIds.length === 0) {
    return {
      tier2Tasks: 0,
      successfulTier2Tasks: 0,
      failedExecutions: 0,
      firstExecutionError: null,
      tasksWithoutCandidates: 0,
      attemptedMutations: 0,
      appliedMutations: 0,
      rejectedMutations: 0,
      pipelineErrors: 0,
      firstRejectedReason: null,
      firstPipelineError: null,
    }
  }

  const executionResults = await Promise.allSettled(
    tier2TaskIds.map((taskId) =>
      executeDomainTask({
        taskId,
        targetProject: input.targetProject,
        topK: input.topK ?? DEFAULT_TOP_K,
        ...input.mcpConfig,
      }),
    ),
  )

  let failedExecutions = 0
  let firstExecutionError: string | null = null
  const successfulTier2TaskIds: string[] = []

  executionResults.forEach((result, index) => {
    if (result.status === 'fulfilled') {
      successfulTier2TaskIds.push(tier2TaskIds[index]!)
      return
    }

    failedExecutions += 1
    if (!firstExecutionError) {
      firstExecutionError = getErrorMessage(result.reason)
    }
  })

  let tasksWithoutCandidates = 0
  let attemptedMutations = 0
  let appliedMutations = 0
  let rejectedMutations = 0
  let pipelineErrors = 0
  let firstRejectedReason: string | null = null
  let firstPipelineError: string | null = null

  for (const tier2TaskId of successfulTier2TaskIds) {
    const candidates = await waitForCandidateMutations(tier2TaskId, attempts, intervalMs)
    if (candidates.length === 0) {
      tasksWithoutCandidates += 1
      continue
    }

    for (const mutation of candidates) {
      attemptedMutations += 1
      try {
        const result = await runMutationPipeline({
          mutationId: mutation.id,
          targetProject: input.targetProject,
          tier1Approved: true,
        })

        if (result.mutation.status === 'applied') {
          appliedMutations += 1
          continue
        }

        rejectedMutations += 1
        if (!firstRejectedReason) {
          const failedStep = getFirstFailedStep(result.steps)
          firstRejectedReason =
            failedStep?.details ??
            result.mutation.rejectionReason ??
            result.task.errorMessage ??
            `Mutation ${result.mutation.id} finished with status '${result.mutation.status}'.`
        }
      } catch (error) {
        pipelineErrors += 1
        if (!firstPipelineError) {
          firstPipelineError = getErrorMessage(error)
        }
      }
    }
  }

  return {
    tier2Tasks: tier2TaskIds.length,
    successfulTier2Tasks: successfulTier2TaskIds.length,
    failedExecutions,
    firstExecutionError,
    tasksWithoutCandidates,
    attemptedMutations,
    appliedMutations,
    rejectedMutations,
    pipelineErrors,
    firstRejectedReason,
    firstPipelineError,
  }
}

export function formatRestartApplyIssue(summary: RestartApplySummary): string | null {
  if (summary.tier2Tasks === 0) {
    return 'Tasks were restarted, but no Tier 2 tasks were available to execute.'
  }

  const issues: string[] = []
  if (summary.failedExecutions > 0) {
    let message = `${summary.failedExecutions} Tier 2 execution(s) failed.`
    if (summary.firstExecutionError) {
      message = `${message} First error: ${summary.firstExecutionError}`
    }
    issues.push(message)
  }

  if (summary.tasksWithoutCandidates > 0) {
    issues.push(`${summary.tasksWithoutCandidates} Tier 2 task(s) did not produce mutation candidates.`)
  }

  if (summary.attemptedMutations === 0) {
    issues.push('No mutation candidates were available to apply.')
  }

  if (summary.rejectedMutations > 0) {
    let message = `${summary.rejectedMutations} mutation pipeline run(s) were rejected.`
    if (summary.firstRejectedReason) {
      message = `${message} First rejection: ${summary.firstRejectedReason}`
    }
    issues.push(message)
  }

  if (summary.pipelineErrors > 0) {
    let message = `${summary.pipelineErrors} mutation pipeline run(s) crashed before completion.`
    if (summary.firstPipelineError) {
      message = `${message} First error: ${summary.firstPipelineError}`
    }
    issues.push(message)
  }

  if (issues.length === 0) {
    return null
  }

  if (summary.appliedMutations > 0) {
    return `Applied ${summary.appliedMutations} mutation(s), but ${issues.join(' ')}`
  }

  return `No mutation was applied. ${issues.join(' ')}`
}
