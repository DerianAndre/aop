import { executeBridgeRequest } from './bridge.js'
import type { BridgeEnvelope, BridgeRequest } from './types.js'

function parseArgs(argv: string[]): BridgeRequest {
  const requestFlagIndex = argv.findIndex((arg) => arg === '--request')
  if (requestFlagIndex === -1 || requestFlagIndex + 1 >= argv.length) {
    throw new Error("Missing '--request' argument")
  }

  try {
    return JSON.parse(argv[requestFlagIndex + 1]) as BridgeRequest
  } catch (error) {
    throw new Error(`Invalid request payload: ${error instanceof Error ? error.message : String(error)}`)
  }
}

function writeResponse(payload: BridgeEnvelope): void {
  process.stdout.write(`${JSON.stringify(payload)}\n`)
}

async function main(): Promise<void> {
  try {
    const request = parseArgs(process.argv.slice(2))
    const data = await executeBridgeRequest(request)
    writeResponse({ ok: true, data })
  } catch (error) {
    writeResponse({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    })
    process.exitCode = 1
  }
}

void main()
