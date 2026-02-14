import { useEffect, useMemo, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { useTargetProjectConfig } from '@/hooks/useTargetProjectConfig'
import {
  archiveTelemetry,
  getModelRegistry,
  getProviderSecretStatus,
  getRuntimeFlags,
  revealProviderSecret,
  setProviderSecret,
  setRuntimeFlags,
} from '@/hooks/useTauri'
import type {
  ArchiveTelemetryResult,
  ModelRegistrySnapshot,
  ModelProfile,
  ProviderSecretStatus,
  RuntimeFlags,
} from '@/types'

function formatProfile(profile: ModelProfile): string {
  const temperature = profile.temperature == null ? 'default' : profile.temperature.toString()
  const maxTokens = profile.maxOutputTokens == null ? 'default' : profile.maxOutputTokens.toString()
  return `${profile.provider}/${profile.modelId} (temp: ${temperature}, max: ${maxTokens})`
}

const DEFAULT_FLAGS: RuntimeFlags = {
  devMode: false,
  modelAdapterEnabled: true,
  modelAdapterStrict: false,
  autoApproveBudgetRequests: true,
  autoCommitMutations: false,
  budgetHeadroomPercent: 25,
  budgetAutoMaxPercent: 40,
  budgetMinIncrement: 250,
  telemetryRetentionDays: 7,
}

const PROVIDER_OPTIONS = ['claude_code', 'openai', 'anthropic', 'gemini', 'xai']

export function SystemView() {
  const { targetProject, mcpCommand, mcpArgs } = useTargetProjectConfig()

  const [registry, setRegistry] = useState<ModelRegistrySnapshot | null>(null)
  const [isLoadingRegistry, setIsLoadingRegistry] = useState(false)
  const [registryError, setRegistryError] = useState<string | null>(null)

  const [flags, setFlags] = useState<RuntimeFlags>(DEFAULT_FLAGS)
  const [isLoadingFlags, setIsLoadingFlags] = useState(false)
  const [isSavingFlags, setIsSavingFlags] = useState(false)
  const [flagsError, setFlagsError] = useState<string | null>(null)
  const [flagsFeedback, setFlagsFeedback] = useState<string | null>(null)

  const [provider, setProvider] = useState('claude_code')
  const [providerStatus, setProviderStatus] = useState<ProviderSecretStatus | null>(null)
  const [isLoadingProviderStatus, setIsLoadingProviderStatus] = useState(false)
  const [providerError, setProviderError] = useState<string | null>(null)
  const [providerFeedback, setProviderFeedback] = useState<string | null>(null)
  const [providerSecret, setProviderSecretValue] = useState('')
  const [sessionToken, setSessionToken] = useState('')
  const [revealedSecret, setRevealedSecret] = useState<string | null>(null)

  const [archiveRetentionDays, setArchiveRetentionDays] = useState(7)
  const [archiveResult, setArchiveResult] = useState<ArchiveTelemetryResult | null>(null)
  const [isArchiving, setIsArchiving] = useState(false)
  const [archiveError, setArchiveError] = useState<string | null>(null)

  useEffect(() => {
    void loadRegistry()
    void loadFlags()
  }, [])

  useEffect(() => {
    setArchiveRetentionDays(flags.telemetryRetentionDays)
  }, [flags.telemetryRetentionDays])

  useEffect(() => {
    void loadProviderStatus(provider)
  }, [provider])

  async function loadRegistry() {
    setIsLoadingRegistry(true)
    setRegistryError(null)
    try {
      const snapshot = await getModelRegistry()
      setRegistry(snapshot)
    } catch (loadError) {
      setRegistryError(loadError instanceof Error ? loadError.message : String(loadError))
    } finally {
      setIsLoadingRegistry(false)
    }
  }

  async function loadFlags() {
    setIsLoadingFlags(true)
    setFlagsError(null)
    try {
      const current = await getRuntimeFlags()
      setFlags(current)
    } catch (loadError) {
      setFlagsError(loadError instanceof Error ? loadError.message : String(loadError))
    } finally {
      setIsLoadingFlags(false)
    }
  }

  async function loadProviderStatus(nextProvider: string) {
    setIsLoadingProviderStatus(true)
    setProviderError(null)
    setRevealedSecret(null)
    try {
      const status = await getProviderSecretStatus({ provider: nextProvider })
      setProviderStatus(status)
    } catch (loadError) {
      setProviderError(loadError instanceof Error ? loadError.message : String(loadError))
    } finally {
      setIsLoadingProviderStatus(false)
    }
  }

  async function handleSaveFlags() {
    setIsSavingFlags(true)
    setFlagsError(null)
    setFlagsFeedback(null)
    try {
      const result = await setRuntimeFlags(flags)
      setFlags(result.flags)
      setFlagsFeedback(
        result.restartRequired
          ? 'Runtime flags updated. Restart required for some changes.'
          : 'Runtime flags updated without restart.',
      )
    } catch (saveError) {
      setFlagsError(saveError instanceof Error ? saveError.message : String(saveError))
    } finally {
      setIsSavingFlags(false)
    }
  }

  async function handleSaveProviderSecret() {
    if (!providerSecret.trim()) {
      setProviderError('Secret value is required.')
      return
    }

    setProviderError(null)
    setProviderFeedback(null)
    try {
      const result = await setProviderSecret({
        provider,
        secret: providerSecret.trim(),
        sessionToken: sessionToken.trim() || undefined,
      })
      if (result.confirmationRequired) {
        setProviderFeedback(
          `Confirmation required. Use session token '${result.confirmationToken ?? 'missing'}' and save again.`,
        )
        if (result.confirmationToken) {
          setSessionToken(result.confirmationToken)
        }
      } else {
        setProviderFeedback(`Secret stored for provider '${provider}'.`)
        setProviderSecretValue('')
      }
      await loadProviderStatus(provider)
    } catch (saveError) {
      setProviderError(saveError instanceof Error ? saveError.message : String(saveError))
    }
  }

  async function handleRevealProviderSecret() {
    setProviderError(null)
    setProviderFeedback(null)
    setRevealedSecret(null)
    try {
      const result = await revealProviderSecret({
        provider,
        sessionToken: sessionToken.trim() || undefined,
      })
      setRevealedSecret(result.secret)
      setProviderFeedback(`Secret revealed for '${provider}'. Developer mode is active.`)
    } catch (revealError) {
      const message = revealError instanceof Error ? revealError.message : String(revealError)
      setProviderError(message)
      const tokenMatch = message.match(/token=([A-Za-z0-9-]+)/)
      if (tokenMatch?.[1]) {
        setSessionToken(tokenMatch[1])
      }
    }
  }

  async function handleArchiveTelemetry() {
    setIsArchiving(true)
    setArchiveError(null)
    try {
      const result = await archiveTelemetry({ retentionDays: archiveRetentionDays })
      setArchiveResult(result)
      await loadFlags()
    } catch (archiveFailure) {
      setArchiveError(archiveFailure instanceof Error ? archiveFailure.message : String(archiveFailure))
    } finally {
      setIsArchiving(false)
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
          <CardTitle>Runtime Flags (.env + live runtime)</CardTitle>
          <div className="flex gap-2">
            <Button disabled={isLoadingFlags} onClick={() => void loadFlags()} size="sm" type="button" variant="outline">
              {isLoadingFlags ? 'Refreshing...' : 'Reload'}
            </Button>
            <Button disabled={isSavingFlags} onClick={() => void handleSaveFlags()} size="sm" type="button">
              {isSavingFlags ? 'Saving...' : 'Save Flags'}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
            <div className="flex items-center justify-between rounded-md border p-3">
              <Label htmlFor="flag-dev-mode">Developer Mode</Label>
              <Switch
                checked={flags.devMode}
                id="flag-dev-mode"
                onCheckedChange={(checked) => setFlags((current) => ({ ...current, devMode: checked }))}
              />
            </div>
            <div className="flex items-center justify-between rounded-md border p-3">
              <Label htmlFor="flag-model-adapter-enabled">Model Adapter Enabled</Label>
              <Switch
                checked={flags.modelAdapterEnabled}
                id="flag-model-adapter-enabled"
                onCheckedChange={(checked) => setFlags((current) => ({ ...current, modelAdapterEnabled: checked }))}
              />
            </div>
            <div className="flex items-center justify-between rounded-md border p-3">
              <Label htmlFor="flag-model-adapter-strict">Model Adapter Strict</Label>
              <Switch
                checked={flags.modelAdapterStrict}
                id="flag-model-adapter-strict"
                onCheckedChange={(checked) => setFlags((current) => ({ ...current, modelAdapterStrict: checked }))}
              />
            </div>
            <div className="flex items-center justify-between rounded-md border p-3">
              <Label htmlFor="flag-auto-approve">Auto Approve Budget</Label>
              <Switch
                checked={flags.autoApproveBudgetRequests}
                id="flag-auto-approve"
                onCheckedChange={(checked) => setFlags((current) => ({ ...current, autoApproveBudgetRequests: checked }))}
              />
            </div>
            <div className="flex items-center justify-between rounded-md border p-3">
              <Label htmlFor="flag-auto-commit">Auto Commit Mutations</Label>
              <Switch
                checked={flags.autoCommitMutations}
                id="flag-auto-commit"
                onCheckedChange={(checked) => setFlags((current) => ({ ...current, autoCommitMutations: checked }))}
              />
            </div>
          </div>

          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
            <div className="space-y-1">
              <Label htmlFor="flag-headroom">Budget Headroom (%)</Label>
              <Input
                id="flag-headroom"
                min={1}
                onChange={(event) =>
                  setFlags((current) => ({ ...current, budgetHeadroomPercent: Number(event.target.value || 0) }))
                }
                step={1}
                type="number"
                value={flags.budgetHeadroomPercent}
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor="flag-auto-max">Budget Auto Max (%)</Label>
              <Input
                id="flag-auto-max"
                min={5}
                onChange={(event) =>
                  setFlags((current) => ({ ...current, budgetAutoMaxPercent: Number(event.target.value || 0) }))
                }
                step={1}
                type="number"
                value={flags.budgetAutoMaxPercent}
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor="flag-min-increment">Budget Min Increment</Label>
              <Input
                id="flag-min-increment"
                min={50}
                onChange={(event) =>
                  setFlags((current) => ({ ...current, budgetMinIncrement: Number(event.target.value || 0) }))
                }
                step={50}
                type="number"
                value={flags.budgetMinIncrement}
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor="flag-retention">Telemetry Retention (days)</Label>
              <Input
                id="flag-retention"
                min={1}
                onChange={(event) =>
                  setFlags((current) => ({ ...current, telemetryRetentionDays: Number(event.target.value || 0) }))
                }
                step={1}
                type="number"
                value={flags.telemetryRetentionDays}
              />
            </div>
          </div>

          {flagsFeedback ? <p className="text-sm">{flagsFeedback}</p> : null}
          {flagsError ? <p className="text-destructive text-sm whitespace-pre-wrap">{flagsError}</p> : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Provider Secrets (Stronghold)</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-[240px_1fr]">
            <div className="space-y-1">
              <Label htmlFor="provider-select">Provider</Label>
              <Select onValueChange={setProvider} value={provider}>
                <SelectTrigger id="provider-select">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PROVIDER_OPTIONS.map((value) => (
                    <SelectItem key={value} value={value}>
                      {value}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="rounded-md border p-3 text-sm">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline">backend {providerStatus?.backend ?? 'stronghold'}</Badge>
                <Badge variant={providerStatus?.configured ? 'default' : 'secondary'}>
                  {providerStatus?.configured ? 'configured' : 'not configured'}
                </Badge>
                <Badge variant={providerStatus?.developerMode ? 'default' : 'secondary'}>
                  dev mode {providerStatus?.developerMode ? 'on' : 'off'}
                </Badge>
                {isLoadingProviderStatus ? <Badge variant="outline">syncing</Badge> : null}
              </div>
              <p className="text-muted-foreground mt-2 text-xs">
                Reveal/edit requires developer mode and a valid per-session confirmation token.
              </p>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div className="space-y-1">
              <Label htmlFor="provider-secret">Secret</Label>
              <Input
                id="provider-secret"
                onChange={(event) => setProviderSecretValue(event.target.value)}
                placeholder="sk-..."
                type="password"
                value={providerSecret}
              />
            </div>
            <div className="space-y-1">
              <Label htmlFor="provider-token">Session Token</Label>
              <Input
                id="provider-token"
                onChange={(event) => setSessionToken(event.target.value)}
                placeholder="Auto-filled when confirmation is required"
                value={sessionToken}
              />
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            <Button onClick={() => void handleSaveProviderSecret()} size="sm" type="button">
              Save Secret
            </Button>
            <Button onClick={() => void handleRevealProviderSecret()} size="sm" type="button" variant="outline">
              Reveal Secret
            </Button>
            <Button
              onClick={() => void loadProviderStatus(provider)}
              size="sm"
              type="button"
              variant="outline"
            >
              Refresh Status
            </Button>
          </div>

          {providerFeedback ? <p className="text-sm whitespace-pre-wrap">{providerFeedback}</p> : null}
          {providerError ? <p className="text-destructive text-sm whitespace-pre-wrap">{providerError}</p> : null}
          {revealedSecret ? (
            <div className="rounded-md border border-amber-400/60 bg-amber-500/10 p-3">
              <p className="text-xs font-medium">Revealed secret</p>
              <p className="font-mono text-xs break-all">{revealedSecret}</p>
            </div>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Telemetry Retention and Archive</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-[220px_1fr]">
            <div className="space-y-1">
              <Label htmlFor="archive-retention">Retention Days</Label>
              <Input
                id="archive-retention"
                min={1}
                onChange={(event) => setArchiveRetentionDays(Number(event.target.value || 0))}
                type="number"
                value={archiveRetentionDays}
              />
            </div>
            <div className="flex items-end">
              <Button disabled={isArchiving} onClick={() => void handleArchiveTelemetry()} type="button">
                {isArchiving ? 'Archiving...' : 'Run Archive Now'}
              </Button>
            </div>
          </div>
          {archiveResult ? (
            <div className="rounded-md border p-3 text-sm">
              <p>retention: {archiveResult.retentionDays} days</p>
              <p>events archived: {archiveResult.eventsArchived}</p>
              <p>runs archived: {archiveResult.runsArchived}</p>
              <p>file: {archiveResult.archiveFile ?? 'none (nothing to archive)'}</p>
            </div>
          ) : null}
          {archiveError ? <p className="text-destructive text-sm whitespace-pre-wrap">{archiveError}</p> : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Model Registry</CardTitle>
          <Button disabled={isLoadingRegistry} onClick={() => void loadRegistry()} size="sm" type="button" variant="outline">
            {isLoadingRegistry ? 'Refreshing...' : 'Refresh'}
          </Button>
        </CardHeader>
        <CardContent className="space-y-4">
          {registryError ? <p className="text-destructive text-sm whitespace-pre-wrap">{registryError}</p> : null}

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

              {registry.loadError ? <p className="text-destructive text-sm">Config load warning: {registry.loadError}</p> : null}

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
