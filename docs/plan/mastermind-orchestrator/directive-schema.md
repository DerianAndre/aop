# Directive Schema v1.0 — Compression Protocol

## Philosophy

The orchestrator (Opus) emits **compressed directives** — maximum semantic density, minimum token count.
Subagents (Haiku/Sonnet) receive these directives and **expand** them into full implementations.

The schema is designed so that **every token from Opus carries architectural weight**.

---

## Directive Format

```yaml
directive:
  id: string          # unique task id (e.g. "auth-001")
  type: enum          # what kind of work (see Task Types)
  priority: 1-5       # 1=critical, 5=nice-to-have
  
  # THE SEED — compressed intent (this is where Opus earns its money)
  seed:
    intent: string    # 1-2 sentence goal
    arch: string[]    # architectural patterns/constraints
    spec: string      # compressed specification (the DSL)
    edges: string[]   # edge cases Opus identified (subagent would miss these)
    anti: string[]    # what NOT to do (negative constraints)
  
  # EXPANSION INSTRUCTIONS
  expand:
    depth: enum       # minimal | standard | thorough
    format: enum      # code | doc | spec | test | analysis
    style: string     # reference to style profile (optional)
  
  # DEPENDENCIES
  deps:
    needs: string[]   # task ids this depends on
    feeds: string[]   # task ids that depend on this
    context: string[] # shared context keys to read
    emits: string[]   # shared context keys to write

  # REVIEW POLICY
  review:
    mode: enum        # none | self | summary | full
    criteria: string  # what to check (compressed)
```

---

## Task Types

| Type | Subagent Role | Expand Means |
|------|--------------|--------------|
| `code` | Coder | Full implementation with imports, types, error handling |
| `test` | Tester | Test suite with edge cases, mocks, assertions |
| `doc` | Documenter | Full documentation, examples, API reference |
| `refactor` | Coder | Transform existing code per constraints |
| `review` | Reviewer | Analysis, issues, suggestions |
| `design` | Architect | Full technical spec from compressed seed |
| `integrate` | Coder | Glue code, adapters, wiring |
| `debug` | Debugger | Root cause analysis, fix, regression test |

---

## Compression DSL — The `spec` Field

This is where the real savings happen. The orchestrator uses a shorthand that subagents are trained to decompose.

### Syntax

```
ENTITY(props) -> BEHAVIOR [CONSTRAINT]
```

### Examples

```
# Auth module with JWT
User(email,pass,role) -> authenticate(credentials):Token [stateless, no-session]
Token(sub,exp,role) -> verify():Claims [reject-expired, refresh-window=5m]
RefreshToken(token,family) -> rotate():NewPair [detect-reuse, revoke-family]

# REST API endpoint  
POST /auth/login <- LoginDTO(email,pass) -> TokenPair [rate-limit=5/min, audit-log]
POST /auth/refresh <- RefreshDTO(token) -> TokenPair [cookie-only, httponly]

# Repository pattern
UserRepo: find(id|email) -> User?, save(User) -> void, delete(id) -> void [soft-delete]
```

### Shorthand Conventions

| Shorthand | Means |
|-----------|-------|
| `->` | produces / returns |
| `<-` | receives / accepts |
| `?` | nullable/optional |
| `[]` | constraints in brackets |
| `\|` | alternatives |
| `...` | subagent decides details |
| `!` | critical / must not fail |
| `~` | approximate / flexible |
| `@` | reference to another task's output |

---

## Edge Cases Field — `edges`

This is Opus's highest-value output. These are things a cheaper model would miss:

```yaml
edges:
  - "token refresh race: two requests with same refresh token arrive simultaneously"
  - "clock skew: JWT exp check must tolerate ±30s"
  - "password hash timing: constant-time comparison to prevent timing attacks"
```

The subagent MUST address every edge case listed. This is non-negotiable.

---

## Anti-Patterns Field — `anti`

Negative constraints prevent the subagent from taking obvious-but-wrong paths:

```yaml
anti:
  - "no ORM in domain layer"
  - "no try-catch swallowing errors silently"  
  - "no hardcoded secrets, even in examples"
  - "no any types in TypeScript"
```

---

## Expansion Depth Levels

| Level | Output Size | Detail |
|-------|------------|--------|
| `minimal` | ~50-200 lines | Core logic only, minimal comments |
| `standard` | ~200-500 lines | Full implementation, JSDoc, error handling |
| `thorough` | ~500+ lines | Everything + examples, edge case tests, docs |

---

## Example: Full Directive

```yaml
directive:
  id: "auth-001"
  type: code
  priority: 1
  
  seed:
    intent: "JWT authentication module for hexagonal architecture"
    arch: ["hexagonal", "port+adapter", "DDD-value-objects"]
    spec: |
      # Domain
      Token(sub,exp,role,jti) -> verify():Claims [pure, no-io]
      Credentials(email,pass) -> validate():self [value-object, immutable]
      
      # Ports
      AuthPort: login(Credentials)->TokenPair, refresh(token)->TokenPair, logout(jti)->void
      HashPort: hash(pass)->hashed, verify(pass,hashed)->bool
      TokenPort: sign(claims)->token, verify(token)->claims
      
      # Adapters (infra)
      BcryptAdapter implements HashPort [rounds=12]
      JwtAdapter implements TokenPort [RS256, key-rotation-ready]
      
    edges:
      - "refresh token reuse detection → revoke entire token family"
      - "concurrent refresh race → only first wins, rest get 401"
      - "JWT jti claim for logout blacklist with TTL = token exp"
      
    anti:
      - "no express/fastify coupling in domain"
      - "no string types where value objects apply"
      - "no synchronous password hashing"
  
  expand:
    depth: standard
    format: code
    style: "clean-ts"
  
  deps:
    needs: []
    feeds: ["api-001", "test-001"]
    context: []
    emits: ["auth-types", "auth-ports"]
  
  review:
    mode: self
    criteria: "ports are pure interfaces, no infra leaks into domain"
```

---

## Orchestrator Output Budget

The orchestrator should target:

| Directive Complexity | Target Tokens (output) |
|---------------------|----------------------|
| Simple task | 100-200 tokens |
| Standard task | 200-400 tokens |
| Complex task | 400-700 tokens |
| Multi-task plan | 500-1000 tokens |

**Rule: If the orchestrator exceeds 1000 output tokens, it's doing the subagent's job.**
