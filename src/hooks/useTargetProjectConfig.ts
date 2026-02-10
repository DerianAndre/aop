import { useEffect, useMemo } from 'react'

import { getDefaultTargetProject } from '@/hooks/useTauri'
import { useAopStore } from '@/store/aop-store'

function parseCommandArgs(rawArgs: string): string[] | undefined {
  const values = rawArgs
    .trim()
    .split(/\s+/)
    .map((value) => value.trim())
    .filter(Boolean)

  return values.length > 0 ? values : undefined
}

export function useTargetProjectConfig() {
  const targetProject = useAopStore((state) => state.targetProject)
  const mcpCommand = useAopStore((state) => state.mcpCommand)
  const mcpArgs = useAopStore((state) => state.mcpArgs)
  const setTargetProject = useAopStore((state) => state.setTargetProject)
  const setMcpCommand = useAopStore((state) => state.setMcpCommand)
  const setMcpArgs = useAopStore((state) => state.setMcpArgs)

  useEffect(() => {
    if (targetProject.trim()) {
      return
    }

    getDefaultTargetProject()
      .then((projectPath) => {
        setTargetProject(projectPath)
      })
      .catch(() => {
        // Keep field user-driven when default path isn't available.
      })
  }, [setTargetProject, targetProject])

  const mcpConfig = useMemo(() => {
    const command = mcpCommand.trim()
    if (!command) {
      return {}
    }

    return {
      mcpCommand: command,
      mcpArgs: parseCommandArgs(mcpArgs),
    }
  }, [mcpArgs, mcpCommand])

  return {
    targetProject,
    setTargetProject,
    mcpCommand,
    setMcpCommand,
    mcpArgs,
    setMcpArgs,
    mcpConfig,
  }
}

