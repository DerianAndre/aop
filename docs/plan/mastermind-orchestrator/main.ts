import { MastermindOrchestrator } from './orchestrator.js';
import { DEFAULT_CONFIG, type ModelConfig } from './types.js';
import { writeFileSync } from 'fs';

// ─── CLI ─────────────────────────────────────────────────────────

const userRequest = process.argv.slice(2).join(' ') || `
Build a URL shortener service with:
- Custom short codes and auto-generated codes
- Click analytics (referrer, device, geo)
- Rate limiting per API key
- Link expiration
- Hexagonal architecture, TypeScript, clean domain
`;

// Optional: override models for testing
const config: ModelConfig = {
  ...DEFAULT_CONFIG,
  // Uncomment to test with cheaper models:
  // orchestrator: { model: 'claude-sonnet-4-5-20250929', maxOutputTokens: 1500 },
  // subagent: { model: 'claude-haiku-4-5-20251001', maxOutputTokens: 8000 },
};

async function main() {
  console.log('╔══════════════════════════════════════════════════╗');
  console.log('║   🧠 MASTERMIND ORCHESTRATOR PIPELINE            ║');
  console.log('╚══════════════════════════════════════════════════╝');
  console.log();
  console.log('📝 User Request:');
  console.log(userRequest.trim());
  console.log();

  const orchestrator = new MastermindOrchestrator(config);

  try {
    const result = await orchestrator.execute(userRequest);

    // Write full output to file
    const outputPath = `./output-${Date.now()}.json`;
    writeFileSync(outputPath, JSON.stringify(result, null, 2));
    console.log(`📁 Full output saved to: ${outputPath}`);

    // Print summary of each subagent's work
    console.log('\n📊 RESULTS SUMMARY\n');
    for (const r of result.results) {
      console.log(`  ${r.directiveId} (${r.role})`);
      console.log(`    Tokens: ${r.tokenUsage.outputTokens} out | Cost: $${r.tokenUsage.cost.toFixed(4)}`);
      console.log(`    Review: ${r.selfReview.result}`);
      console.log(`    Output: ${r.output.length} chars`);
      console.log();
    }
  } catch (error) {
    console.error('❌ Pipeline failed:', error);
    process.exit(1);
  }
}

main();
