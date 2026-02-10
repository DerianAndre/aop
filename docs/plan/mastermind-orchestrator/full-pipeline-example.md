# Example: Full Pipeline Flow

## User Request
> "Build a URL shortener with custom codes, click analytics, rate limiting, and link expiration. Hexagonal architecture, TypeScript."

---

## Phase 1: Orchestrator Output (Opus — ~450 tokens)

```yaml
plan:
  summary: "url-shortener: domain→analytics→rate-limiter→API, hexagonal+DDD"
  tasks: 4
  parallel_groups: [["url-002","url-003"], ["url-004"]]

directives:
  - directive:
      id: url-001
      type: code
      priority: 1
      seed:
        intent: "URL shortener domain layer — entities, value objects, ports"
        arch: [hexagonal, DDD, value-objects]
        spec: |
          ShortUrl(id,originalUrl,code,createdBy,expiresAt?,clickCount) 
          UrlCode(value:string) -> validate():self [6-8 alphanum, no ambiguous chars(0OlI)]
          OriginalUrl(value:string) -> validate():self [must-parse, https-only-prod]
          
          ShortenPort: create(url,code?,ttl?)->ShortUrl, resolve(code)->OriginalUrl, delete(id)->void
          AnalyticsPort: record(click:ClickEvent)->void, stats(urlId,range)->ClickStats
          RateLimitPort: check(apiKey,action)->allowed:bool [token-bucket]
        edges:
          - "code collision on auto-generate — retry with backoff, max 5 attempts"
          - "expired URL resolve — return gone(410) not not-found(404)"
          - "race condition: two creates with same custom code simultaneously"
        anti:
          - "no URL validation with regex — use URL constructor"
          - "no click counting on resolve path — async event, don't block redirect"
      expand: { depth: standard, format: code }
      deps: { needs: [], feeds: [url-002,url-003,url-004], emits: [url-types] }
      review: { mode: self, criteria: "domain has zero infra imports" }

  - directive:
      id: url-002
      type: code
      priority: 2
      seed:
        intent: "click analytics adapter — record and query"
        arch: [adapter, event-driven]
        spec: |
          ClickEvent(urlId,timestamp,ip,userAgent,referer)
          ClickStats(total,byDay:[{date,count}],topReferers:[{url,count}],devices:{mobile,desktop,tablet})
          AnalyticsAdapter implements AnalyticsPort [append-only-log, batch-flush-interval=5s]
        edges:
          - "geo lookup from IP — async, don't block, graceful fallback to 'unknown'"
          - "user-agent parsing — use ua-parser-js, cache parsed results"
          - "high cardinality referers — cap at top 100, aggregate rest as 'other'"
        anti:
          - "no storing raw IP long-term — hash after geo lookup for privacy"
      expand: { depth: standard, format: code }
      deps: { needs: [url-001], context: [url-types] }
      review: { mode: self }

  - directive:
      id: url-003
      type: code
      priority: 2
      seed:
        intent: "rate limiter adapter — token bucket per API key"
        arch: [adapter, token-bucket]
        spec: |
          RateLimitAdapter implements RateLimitPort [in-memory-default, redis-optional]
          Config: { maxTokens:100, refillRate:10/s, windowSize:1m }
        edges:
          - "distributed rate limiting — if redis unavailable, fall back to local with warning"
          - "clock drift between instances — use token timestamp, not wall clock"
        anti:
          - "no fixed-window — use sliding window or token bucket"
      expand: { depth: minimal, format: code }
      deps: { needs: [url-001], context: [url-types] }
      review: { mode: self }

  - directive:
      id: url-004
      type: integrate
      priority: 3
      seed:
        intent: "HTTP API layer — routes, middleware, composition root"
        arch: [express-adapter, composition-root]
        spec: |
          POST /api/shorten <- {url,code?,ttl?} -> {shortUrl,code,expiresAt?} [auth, rate-limit]
          GET /:code -> 302 redirect [no-auth, record-click-async]
          GET /api/stats/:code -> ClickStats [auth]
          DELETE /api/urls/:id -> 204 [auth, owner-only]
          
          CompositionRoot: wire all ports→adapters, env-based config
        edges:
          - "301 vs 302 redirect — use 302, short URLs may change targets later"
          - "HEAD request on /:code — respond without recording click"
        anti:
          - "no domain logic in route handlers — call application service only"
      expand: { depth: standard, format: code }
      deps: { needs: [url-001,url-002,url-003], context: [url-types] }
      review: { mode: self, criteria: "handlers are thin, all logic in services" }

context_notes: "url-types context = all domain interfaces and value objects from url-001"
```

**Opus total output: ~450 tokens. Contains ALL architectural decisions, edge cases, constraints.**

---

## Phase 2: Subagent Expansion (Haiku — ~3000 tokens per task)

Each subagent takes the compressed directive and produces full implementation.

### url-001 Subagent Output (abbreviated)

```typescript
// domain/value-objects/url-code.ts
export class UrlCode {
  private static readonly ALPHABET = 'abcdefghjkmnpqrstuvwxyz23456789'; // no 0,O,l,I,1
  private static readonly MIN_LENGTH = 6;
  private static readonly MAX_LENGTH = 8;
  
  private constructor(public readonly value: string) {}
  
  static create(value: string): UrlCode {
    if (value.length < this.MIN_LENGTH || value.length > this.MAX_LENGTH) {
      throw new InvalidUrlCodeError(`Code must be ${this.MIN_LENGTH}-${this.MAX_LENGTH} chars`);
    }
    if (!/^[a-hjkmnp-z2-9]+$/.test(value)) {
      throw new InvalidUrlCodeError('Code contains ambiguous characters');
    }
    return new UrlCode(value);
  }
  
  static generate(): UrlCode {
    const length = this.MIN_LENGTH;
    let code = '';
    for (let i = 0; i < length; i++) {
      code += this.ALPHABET[Math.floor(Math.random() * this.ALPHABET.length)];
    }
    return new UrlCode(code);
  }
  
  equals(other: UrlCode): boolean {
    return this.value === other.value;
  }
}

// ... (full implementation continues for all entities, ports, etc.)
```

### Edge Cases Addressed ✅

```
✅ "code collision on auto-generate" 
   → ShortenService.create() has retry loop with max 5 attempts, exponential backoff

✅ "expired URL resolve" 
   → ResolveService checks expiresAt, throws UrlExpiredError (maps to 410 in API layer)

✅ "race condition: two creates with same custom code"
   → ShortenPort.create() contract requires unique constraint, adapter throws CodeConflictError
```

---

## Phase 3: Cost Comparison

| Approach | Tokens | Cost |
|----------|--------|------|
| **All Opus** | ~4000 in, ~12000 out | ~$0.96 |
| **Hybrid Pipeline** | | |
| ├─ Opus orchestrator | ~2000 in, ~450 out | ~$0.064 |
| ├─ Haiku ×4 subagents | ~4000 in, ~12000 out | ~$0.051 |
| └─ (no review pass) | | |
| **Hybrid Total** | | **~$0.115** |
| **Savings** | | **~88%** |

Same architectural quality. ~88% cheaper.
