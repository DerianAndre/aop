# 🧠 Mastermind Orchestrator

**Maximum complexity, minimum cost.**

An orchestration pattern where an expensive model (Opus) does the thinking and a cheap model (Haiku) does the building. Opus emits compressed architectural directives; subagents expand them into full implementations.

## The Idea

```
Traditional:  Opus thinks + Opus writes = $$$$$
Mastermind:   Opus thinks + Haiku writes = $     (same quality, ~88% cheaper)
```

The expensive model pays for **reasoning depth** — edge cases, architecture, constraints.
The cheap model pays for **token volume** — expanding seeds into full code.

## Architecture

```
User Request ──→ Orchestrator (Opus, ≤1000 tokens out)
                      │
                      ├─→ Directive 1 ──→ Subagent (Haiku) ──→ Full implementation
                      ├─→ Directive 2 ──→ Subagent (Haiku) ──→ Full implementation  
                      └─→ Directive 3 ──→ Subagent (Haiku) ──→ Full implementation
                                                                      │
                                              Optional: Orchestrator reviews summary
```

## Project Structure

```
brainstorm-orchestrator/
├── schemas/
│   └── directive-schema.md    # The compression protocol (the contract)
├── prompts/
│   ├── orchestrator-system.md # Opus system prompt (think deep, write short)
│   ├── subagent-base.md       # Base decompressor prompt
│   └── subagent-roles.md      # Role-specific overlays (coder, tester, etc.)
├── src/
│   ├── types.ts               # TypeScript types for the schema
│   ├── orchestrator.ts        # Pipeline engine
│   └── main.ts                # CLI entry point
└── examples/
    └── full-pipeline-example.md  # End-to-end worked example
```

## How It Works

### 1. Compression Protocol

The orchestrator uses a DSL to compress specifications:

```
Entity(props) -> behavior():Return [constraints]
Port: method(input)->output [constraints]
Adapter implements Port [config]
VERB /route <- InputDTO -> OutputDTO [middleware]
```

### 2. High-Value Orchestrator Output

Opus focuses on what cheaper models miss:
- **`edges`**: Race conditions, security holes, boundary cases
- **`anti`**: Patterns that seem right but are wrong
- **`arch`**: Architectural constraints that prevent drift

### 3. Cheap Expansion

Haiku receives the compressed directive + a role-specific prompt and expands it into complete, production-quality code.

### 4. Self-Review

Subagents validate their own output against the directive's criteria, edge cases, and anti-patterns before responding.

## Usage

```bash
# Install dependencies
npm install @anthropic-ai/sdk

# Run with a custom request
npx tsx src/main.ts "Build a notification system with email, SMS, push, user prefs, rate limiting"

# Or use the default example
npx tsx src/main.ts
```

## Configuration

Edit `src/types.ts` to change model assignments:

```typescript
const config: ModelConfig = {
  orchestrator: { model: 'claude-opus-4-6', maxOutputTokens: 1000 },
  subagent:     { model: 'claude-haiku-4-5-20251001', maxOutputTokens: 8000 },
  reviewer:     { model: 'claude-sonnet-4-5-20250929', maxOutputTokens: 2000 },
};
```

## Cost Comparison

For a typical 4-task decomposition producing ~12K tokens of code:

| Approach | Cost | Quality |
|----------|------|---------|
| All Opus | ~$0.96 | ⭐⭐⭐⭐⭐ |
| Mastermind (Opus+Haiku) | ~$0.12 | ⭐⭐⭐⭐½ |
| All Haiku | ~$0.05 | ⭐⭐⭐ |

The half-star difference is the trade: subagents occasionally miss nuance that Opus would catch inline. But Opus's edge cases and anti-patterns close most of that gap.

## Next Steps

- [ ] Proper YAML parser for orchestrator output (currently JSON fallback)
- [ ] Structured context passing between subagents
- [ ] Opus review pass for critical tasks
- [ ] Patch directive flow (Opus corrects subagent mistakes)
- [ ] Streaming output for long-running pipelines
- [ ] Prompt caching for subagent system prompts (cuts input cost further)
- [ ] Integration with Claude Code for direct file writing
