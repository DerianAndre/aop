import { useEffect, useMemo, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useTargetProjectConfig } from '@/hooks/useTargetProjectConfig'
import { getModelRegistry } from '@/hooks/useTauri'
import type { ModelRegistrySnapshot, ModelProfile } from '@/types'

function formatProfile(profile: ModelProfile): string {
  const temperature = profile.temperature == null ? 'default' : profile.temperature.toString()
  const maxTokens = profile.maxOutputTokens == null ? 'default' : profile.maxOutputTokens.toString()
  return `${profile.provider}/${profile.modelId} (temp: ${temperature}, max: ${maxTokens})`
}

export function SystemView() {
  const { targetProject, mcpCommand, mcpArgs } = useTargetProjectConfig()
  const [registry, setRegistry] = useState<ModelRegistrySnapshot | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void loadRegistry()
  }, [])

  async function loadRegistry() {
    setIsLoading(true)
    setError(null)
    try {
      const snapshot = await getModelRegistry()
      setRegistry(snapshot)
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError))
    } finally {
      setIsLoading(false)
    }
  }

  const tierEntries = useMemo(
    () =>
      registry?.config.tiers
        ? Object.entries(registry.config.tiers).sort(
            (left: [string, ModelProfile[]], right: [string, ModelProfile[]]) => Number(left[0]) - Number(right[0]),
          )
        : [],
    [registry?.config.tiers],
  )

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Model Registry</CardTitle>
          <Button disabled={isLoading} onClick={() => void loadRegistry()} size="sm" type="button" variant="outline">
            {isLoading ? 'Refreshing...' : 'Refresh'}
          </Button>
        </CardHeader>
        <CardContent className="space-y-4">
          {error ? <p className="text-destructive text-sm">{error}</p> : null}

          {registry ? (
            <>
              <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
                <div className="rounded-md border p-3">
                  <p className="text-muted-foreground text-xs">Config Path</p>
                  <p className="text-sm break-all">{registry.configPath}</p>
                </div>
                <div className="rounded-md border p-3">
                  <p className="text-muted-foreground text-xs">Loaded From File</p>
                  <p className="text-sm">{registry.loadedFromFile ? 'Yes' : 'No (defaults)'}</p>
                </div>
                <div className="rounded-md border p-3">
                  <p className="text-muted-foreground text-xs">Default Provider</p>
                  <p className="text-sm">{registry.config.defaultProvider}</p>
                </div>
              </div>

              {registry.loadError ? (
                <p className="text-destructive text-sm">Config load warning: {registry.loadError}</p>
              ) : null}

              <div className="space-y-3">
                <h3 className="text-sm font-semibold">Tier Routing</h3>
                {tierEntries.map(([tier, profiles]) => (
                  <div className="rounded-md border p-3" key={tier}>
                    <p className="mb-2 text-sm font-medium">Tier {tier}</p>
                    <div className="flex flex-wrap gap-2">
                      {profiles.map((profile, index) => (
                        <Badge key={`${tier}-${profile.provider}-${profile.modelId}-${index}`} variant="secondary">
                          {formatProfile(profile)}
                        </Badge>
                      ))}
                    </div>
                  </div>
                ))}
              </div>

              <div className="space-y-3">
                <h3 className="text-sm font-semibold">Persona Overrides</h3>
                {Object.entries(registry.config.personaOverrides).length > 0 ? (
                  Object.entries(registry.config.personaOverrides).map(([persona, profiles]) => (
                    <div className="rounded-md border p-3" key={persona}>
                      <p className="mb-2 text-sm font-medium">{persona}</p>
                      <div className="flex flex-wrap gap-2">
                        {profiles.map((profile, index) => (
                          <Badge key={`${persona}-${profile.provider}-${profile.modelId}-${index}`} variant="outline">
                            {formatProfile(profile)}
                          </Badge>
                        ))}
                      </div>
                    </div>
                  ))
                ) : (
                  <p className="text-muted-foreground text-sm">No persona overrides configured.</p>
                )}
              </div>
            </>
          ) : (
            <p className="text-muted-foreground text-sm">No registry snapshot loaded yet.</p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Runtime Connection Context</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2 text-sm">
          <p>
            <span className="text-muted-foreground">Target project:</span> {targetProject || 'not set'}
          </p>
          <p>
            <span className="text-muted-foreground">MCP command:</span> {mcpCommand || 'default bridge behavior'}
          </p>
          <p>
            <span className="text-muted-foreground">MCP args:</span> {mcpArgs || 'none'}
          </p>
        </CardContent>
      </Card>
    </div>
  )
}
