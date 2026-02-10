// Directive Schema Types

export type TaskType = 'code' | 'test' | 'doc' | 'refactor' | 'review' | 'design' | 'integrate' | 'debug';
export type ExpandDepth = 'minimal' | 'standard' | 'thorough';
export type ExpandFormat = 'code' | 'doc' | 'spec' | 'test' | 'analysis';
export type ReviewMode = 'none' | 'self' | 'summary' | 'full';
export type SubagentRole = 'coder' | 'tester' | 'documenter' | 'reviewer' | 'integrator';
export type ModelTier = 'opus' | 'sonnet' | 'haiku';

export interface DirectiveSeed {
  intent: string;
  arch: string[];
  spec: string;
  edges: string[];
  anti: string[];
}

export interface DirectiveExpand {
  depth: ExpandDepth;
  format: ExpandFormat;
  style?: string;
}

export interface DirectiveDeps {
  needs: string[];
  feeds: string[];
  context: string[];
  emits: string[];
}

export interface DirectiveReview {
  mode: ReviewMode;
  criteria?: string;
}

export interface Directive {
  id: string;
  type: TaskType;
  priority: number;
  seed: DirectiveSeed;
  expand: DirectiveExpand;
  deps: DirectiveDeps;
  review: DirectiveReview;
}

export interface OrchestratorPlan {
  summary: string;
  tasks: number;
  parallel_groups: string[][];
  directives: Directive[];
  context_notes?: string;
}

export interface SubagentResult {
  directiveId: string;
  role: SubagentRole;
  output: string;
  edgesCovered: string[];
  selfReview: {
    criteria: string;
    result: 'PASS' | 'FAIL';
    notes: string;
  };
  contextEmitted: Record<string, string>;
  escalations: Escalation[];
  tokenUsage: TokenUsage;
}

export interface Escalation {
  taskId: string;
  issue: string;
  suggestion: string;
}

export interface TokenUsage {
  inputTokens: number;
  outputTokens: number;
  model: string;
  cost: number;
}

export interface PipelineResult {
  plan: OrchestratorPlan;
  results: SubagentResult[];
  review?: OrchestratorReview;
  totalCost: CostBreakdown;
}

export interface OrchestratorReview {
  verdict: 'approved' | 'needs-patches';
  patches: Directive[];
  notes: string;
}

export interface CostBreakdown {
  orchestratorCost: number;
  subagentCost: number;
  reviewCost: number;
  totalCost: number;
  tokensSaved: number;
  savingsPercent: number;
}

// Model configuration
export interface ModelConfig {
  orchestrator: {
    model: string;
    maxOutputTokens: number;
  };
  subagent: {
    model: string;
    maxOutputTokens: number;
  };
  reviewer: {
    model: string;
    maxOutputTokens: number;
  };
}

// Pricing per million tokens (as of 2025)
export const MODEL_PRICING: Record<string, { input: number; output: number }> = {
  'claude-opus-4-6':          { input: 15.0,  output: 75.0  },
  'claude-sonnet-4-5-20250929': { input: 3.0,   output: 15.0  },
  'claude-haiku-4-5-20251001':  { input: 0.80,  output: 4.0   },
};

export const DEFAULT_CONFIG: ModelConfig = {
  orchestrator: {
    model: 'claude-opus-4-6',
    maxOutputTokens: 1000,   // Hard cap — forces compression
  },
  subagent: {
    model: 'claude-haiku-4-5-20251001',
    maxOutputTokens: 8000,   // Cheap expansion
  },
  reviewer: {
    model: 'claude-sonnet-4-5-20250929',  // Middle ground for review
    maxOutputTokens: 2000,
  },
};

// Role → TaskType mapping
export const ROLE_MAP: Record<TaskType, SubagentRole> = {
  code: 'coder',
  test: 'tester',
  doc: 'documenter',
  refactor: 'coder',
  review: 'reviewer',
  design: 'coder',
  integrate: 'integrator',
  debug: 'coder',
};
