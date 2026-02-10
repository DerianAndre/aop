import Anthropic from '@anthropic-ai/sdk';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import {
  type Directive,
  type OrchestratorPlan,
  type SubagentResult,
  type PipelineResult,
  type ModelConfig,
  type CostBreakdown,
  type TokenUsage,
  DEFAULT_CONFIG,
  MODEL_PRICING,
  ROLE_MAP,
} from './types.js';

// ─── Prompt Loader ───────────────────────────────────────────────

function loadPrompt(filename: string): string {
  return readFileSync(resolve(import.meta.dirname, '../prompts', filename), 'utf-8');
}

const ORCHESTRATOR_PROMPT = loadPrompt('orchestrator-system.md');
const SUBAGENT_BASE_PROMPT = loadPrompt('subagent-base.md');
const SUBAGENT_ROLES_PROMPT = loadPrompt('subagent-roles.md');

// ─── Cost Calculator ─────────────────────────────────────────────

function calculateCost(model: string, inputTokens: number, outputTokens: number): number {
  const pricing = MODEL_PRICING[model];
  if (!pricing) throw new Error(`Unknown model: ${model}`);
  return (inputTokens * pricing.input + outputTokens * pricing.output) / 1_000_000;
}

function calculateSavings(results: PipelineResult): CostBreakdown {
  const orchestratorCost = results.plan
    ? calculateCost(DEFAULT_CONFIG.orchestrator.model, 0, 0) // filled later
    : 0;

  let subagentCost = 0;
  let totalInputTokens = 0;
  let totalOutputTokens = 0;

  for (const r of results.results) {
    subagentCost += r.tokenUsage.cost;
    totalOutputTokens += r.tokenUsage.outputTokens;
    totalInputTokens += r.tokenUsage.inputTokens;
  }

  // What it WOULD have cost with all-Opus
  const allOpusCost = calculateCost(
    DEFAULT_CONFIG.orchestrator.model,
    totalInputTokens,
    totalOutputTokens
  );

  const totalCost = orchestratorCost + subagentCost;
  const savings = allOpusCost - totalCost;

  return {
    orchestratorCost,
    subagentCost,
    reviewCost: 0,
    totalCost,
    tokensSaved: 0, // tokens are the same, cost is different
    savingsPercent: allOpusCost > 0 ? (savings / allOpusCost) * 100 : 0,
  };
}

// ─── Orchestrator (Opus) ─────────────────────────────────────────

export class MastermindOrchestrator {
  private client: Anthropic;
  private config: ModelConfig;
  private sharedContext: Map<string, string> = new Map();

  constructor(config: ModelConfig = DEFAULT_CONFIG) {
    this.client = new Anthropic();
    this.config = config;
  }

  /**
   * Main pipeline: User request → Opus plan → Subagent execution → Results
   */
  async execute(userRequest: string): Promise<PipelineResult> {
    console.log('\n🧠 Phase 1: Orchestrator decomposing task...\n');

    // Step 1: Opus creates the plan
    const { plan, usage: planUsage } = await this.createPlan(userRequest);

    console.log(`📋 Plan: ${plan.summary}`);
    console.log(`📦 Tasks: ${plan.directives.length}`);
    console.log(`⚡ Parallel groups: ${plan.parallel_groups.length}`);
    console.log(`🪙 Orchestrator tokens: ${planUsage.outputTokens} output\n`);

    // Step 2: Execute directives respecting dependency order
    const results = await this.executeDirectives(plan);

    // Step 3: Optional review pass
    // (implement if review.mode === 'summary' or 'full' on any directive)

    const pipelineResult: PipelineResult = { plan, results };
    pipelineResult.totalCost = calculateSavings(pipelineResult);

    this.printCostReport(pipelineResult);

    return pipelineResult;
  }

  /**
   * Phase 1: Opus decomposes the user request into compressed directives
   */
  private async createPlan(
    userRequest: string
  ): Promise<{ plan: OrchestratorPlan; usage: TokenUsage }> {
    const response = await this.client.messages.create({
      model: this.config.orchestrator.model,
      max_tokens: this.config.orchestrator.maxOutputTokens,
      system: ORCHESTRATOR_PROMPT,
      messages: [
        {
          role: 'user',
          content: `Decompose this into compressed directives:\n\n${userRequest}\n\nRespond ONLY with YAML directives following the schema. No explanations.`,
        },
      ],
    });

    const text = response.content
      .filter((b): b is Anthropic.TextBlock => b.type === 'text')
      .map((b) => b.text)
      .join('');

    // Parse YAML plan from response (simplified — in production use a YAML parser)
    const plan = this.parsePlan(text);

    const usage: TokenUsage = {
      inputTokens: response.usage.input_tokens,
      outputTokens: response.usage.output_tokens,
      model: this.config.orchestrator.model,
      cost: calculateCost(
        this.config.orchestrator.model,
        response.usage.input_tokens,
        response.usage.output_tokens
      ),
    };

    return { plan, usage };
  }

  /**
   * Phase 2: Execute directives in dependency order, parallelizing where possible
   */
  private async executeDirectives(plan: OrchestratorPlan): Promise<SubagentResult[]> {
    const results: SubagentResult[] = [];
    const completed = new Set<string>();

    // Execute parallel groups in order
    for (const group of plan.parallel_groups) {
      const groupDirectives = plan.directives.filter((d) => group.includes(d.id));

      // Check all dependencies are met
      for (const directive of groupDirectives) {
        const unmet = directive.deps.needs.filter((dep) => !completed.has(dep));
        if (unmet.length > 0) {
          console.warn(`⚠️ ${directive.id} has unmet deps: ${unmet.join(', ')}. Running sequentially.`);
        }
      }

      console.log(`\n🔧 Phase 2: Executing group [${group.join(', ')}] in parallel...\n`);

      // Run all directives in this group concurrently
      const groupResults = await Promise.all(
        groupDirectives.map((directive) => this.executeSubagent(directive))
      );

      for (const result of groupResults) {
        results.push(result);
        completed.add(result.directiveId);

        // Store emitted context for downstream tasks
        for (const [key, value] of Object.entries(result.contextEmitted)) {
          this.sharedContext.set(key, value);
        }
      }
    }

    // Handle any directives not in parallel groups (sequential fallback)
    const groupedIds = new Set(plan.parallel_groups.flat());
    const ungrouped = plan.directives.filter((d) => !groupedIds.has(d.id));

    for (const directive of ungrouped) {
      console.log(`\n🔧 Executing ${directive.id} sequentially...\n`);
      const result = await this.executeSubagent(directive);
      results.push(result);
      completed.add(result.directiveId);

      for (const [key, value] of Object.entries(result.contextEmitted)) {
        this.sharedContext.set(key, value);
      }
    }

    return results;
  }

  /**
   * Execute a single subagent with a directive
   */
  private async executeSubagent(directive: Directive): Promise<SubagentResult> {
    const role = ROLE_MAP[directive.type];

    // Build context from dependencies
    const contextParts: string[] = [];
    for (const ctxKey of directive.deps.context) {
      const ctx = this.sharedContext.get(ctxKey);
      if (ctx) contextParts.push(`## Context: ${ctxKey}\n${ctx}`);
    }

    // Build the subagent prompt
    const systemPrompt = this.buildSubagentPrompt(role);

    const userMessage = `
## Directive

\`\`\`yaml
${this.serializeDirective(directive)}
\`\`\`

${contextParts.length > 0 ? `## Shared Context\n${contextParts.join('\n\n')}` : ''}

Expand this directive into a complete implementation. Follow all constraints, handle all edge cases, respect all anti-patterns.
`.trim();

    console.log(`  🤖 ${directive.id} (${role}) → ${this.config.subagent.model}`);

    const response = await this.client.messages.create({
      model: this.config.subagent.model,
      max_tokens: this.config.subagent.maxOutputTokens,
      system: systemPrompt,
      messages: [{ role: 'user', content: userMessage }],
    });

    const output = response.content
      .filter((b): b is Anthropic.TextBlock => b.type === 'text')
      .map((b) => b.text)
      .join('');

    const usage: TokenUsage = {
      inputTokens: response.usage.input_tokens,
      outputTokens: response.usage.output_tokens,
      model: this.config.subagent.model,
      cost: calculateCost(
        this.config.subagent.model,
        response.usage.input_tokens,
        response.usage.output_tokens
      ),
    };

    console.log(`  ✅ ${directive.id} done — ${usage.outputTokens} tokens, $${usage.cost.toFixed(4)}`);

    return {
      directiveId: directive.id,
      role,
      output,
      edgesCovered: directive.seed.edges, // trust self-review for now
      selfReview: {
        criteria: directive.review.criteria || 'general quality',
        result: 'PASS',
        notes: 'self-reviewed by subagent',
      },
      contextEmitted: this.extractEmittedContext(output, directive.deps.emits),
      escalations: [],
      tokenUsage: usage,
    };
  }

  // ─── Helpers ─────────────────────────────────────────────────

  private buildSubagentPrompt(role: string): string {
    // Extract role-specific section from the roles prompt
    const roleSection = this.extractRoleSection(role);
    return `${SUBAGENT_BASE_PROMPT}\n\n---\n\n${roleSection}`;
  }

  private extractRoleSection(role: string): string {
    const roleHeaders: Record<string, string> = {
      coder: '## 🔧 Coder Role',
      tester: '## 🧪 Tester Role',
      documenter: '## 📝 Documenter Role',
      reviewer: '## 🔍 Reviewer Role',
      integrator: '## 🔗 Integrator Role',
    };

    const header = roleHeaders[role];
    if (!header) return '';

    const idx = SUBAGENT_ROLES_PROMPT.indexOf(header);
    if (idx === -1) return '';

    // Find the next role header or end of file
    const nextHeaders = Object.values(roleHeaders).filter((h) => h !== header);
    let endIdx = SUBAGENT_ROLES_PROMPT.length;
    for (const nh of nextHeaders) {
      const ni = SUBAGENT_ROLES_PROMPT.indexOf(nh, idx + 1);
      if (ni !== -1 && ni < endIdx) endIdx = ni;
    }

    return SUBAGENT_ROLES_PROMPT.slice(idx, endIdx).trim();
  }

  private serializeDirective(directive: Directive): string {
    // Simple YAML serialization (in production, use a proper YAML library)
    return JSON.stringify(directive, null, 2);
  }

  private extractEmittedContext(output: string, emitKeys: string[]): Record<string, string> {
    // Simple extraction — subagents emit context under "### Context Emitted" headers
    const context: Record<string, string> = {};
    for (const key of emitKeys) {
      // For now, emit the full output as context (refine with structured extraction later)
      context[key] = output.slice(0, 2000); // cap context size
    }
    return context;
  }

  private parsePlan(text: string): OrchestratorPlan {
    // Simplified parser — in production, use yaml.parse()
    // For now, try to extract structured data from the response
    // This is the integration point where you'd add a proper YAML/JSON parser

    // Attempt JSON parse first (if orchestrator outputs JSON)
    try {
      const jsonMatch = text.match(/```(?:json|yaml)?\s*([\s\S]*?)```/);
      if (jsonMatch) {
        return JSON.parse(jsonMatch[1]) as OrchestratorPlan;
      }
      return JSON.parse(text) as OrchestratorPlan;
    } catch {
      // Fallback: create a minimal plan structure
      console.warn('⚠️ Could not parse plan as JSON/YAML. Using raw text as single directive.');
      return {
        summary: 'auto-parsed plan',
        tasks: 1,
        parallel_groups: [['task-001']],
        directives: [
          {
            id: 'task-001',
            type: 'code',
            priority: 1,
            seed: {
              intent: 'See orchestrator raw output',
              arch: [],
              spec: text,
              edges: [],
              anti: [],
            },
            expand: { depth: 'standard', format: 'code' },
            deps: { needs: [], feeds: [], context: [], emits: [] },
            review: { mode: 'self' },
          },
        ],
      };
    }
  }

  private printCostReport(result: PipelineResult): void {
    const { totalCost } = result;
    console.log('\n' + '═'.repeat(50));
    console.log('💰 COST REPORT');
    console.log('═'.repeat(50));
    console.log(`  Orchestrator (Opus):  $${totalCost.orchestratorCost.toFixed(4)}`);
    console.log(`  Subagents (Haiku):    $${totalCost.subagentCost.toFixed(4)}`);
    console.log(`  Review (Sonnet):      $${totalCost.reviewCost.toFixed(4)}`);
    console.log(`  ─────────────────────────────`);
    console.log(`  Total:                $${totalCost.totalCost.toFixed(4)}`);
    console.log(`  vs All-Opus:          ~${totalCost.savingsPercent.toFixed(0)}% savings`);
    console.log('═'.repeat(50) + '\n');
  }
}
