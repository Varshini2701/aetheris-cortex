# Implementation Guide: P2 — Request Classifier

## 1. Architectural Design & Trait Interface

The request classifier is model-agnostic, decoupled from concrete provider APIs, and loaded once at router startup to avoid per-request client allocation overhead.

### Data Structures & Trait (`llm-router/src/routing/classifier.rs`)

```rust
#[derive(Debug, Clone)]
pub struct ClassifyInput {
    pub query: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub request_type: RequestType,
    pub complexity: u8,
    pub confidence: f32,
}

#[async_trait]
pub trait RequestClassifier: Send + Sync {
    async fn classify(&self, input: ClassifyInput) -> Classification;
}
```

### Supported Request Types:
- `code_generation`
- `code_understanding`
- `technical_design`
- `analytical_reasoning`
- `writing`
- `factual_lookup`
- `general`

### Bounds:
- `complexity`: clamped to integer range `1..=5`
- `confidence`: clamped to float range `0.0..=1.0`

---

## 2. Classifier Backends & Fallback Strategy

### Regex Classifier (`RegexClassifier`)
- Built-in, zero-latency keyword pattern voting.
- Serves as the safe baseline implementation.
- Returns `complexity: 3`, `confidence: 0.0`.

### Hosted / OpenAI-Compatible Classifier (`HostedClassifier`)
- Targets OpenAI-compatible chat completion APIs (including Groq: `https://api.groq.com/openai/v1/chat/completions`).
- Employs zero-temperature instruction prompting requesting structured JSON classification output.
- Robust JSON parsing: strips optional markdown fences (````json ... ```` or ```` ... ````).
- **Graceful Fallback:** If any of the following occur:
  - Client initialization failure
  - Network error / connection drop
  - Request timeout
  - Non-200 HTTP response
  - Empty choice list
  - Malformed / non-JSON content
  - Unknown request type string
  - Non-finite confidence (`NaN`, `±Inf`)
  
  The classifier automatically falls back to `RegexClassifier` on the current query without panicking or interrupting traffic.

---

## 3. Configuration Layer (`llm-router/src/config.rs`)

Configuration is managed via `GatewayConfig` and environment variables, keeping credentials and endpoints out of source code:

| Setting | Field | Env Var | Default |
|---|---|---|---|
| Backend | `classifier_backend` | `CLASSIFIER_BACKEND` | `"regex"` |
| Model | `classifier_model` | `CLASSIFIER_MODEL` | `"openai/gpt-oss-20b"` |
| Endpoint | `classifier_endpoint` | `CLASSIFIER_ENDPOINT` | `"https://api.groq.com/openai/v1/chat/completions"` |
| Timeout | `classifier_timeout_secs` | `CLASSIFIER_TIMEOUT_SECS` | `5` |
| API Key | `classifier_api_key` | `GROQ_API_KEY` | `""` |

---

## 4. Routing Pipeline Integration (`llm-router/src/routing/mod.rs`)

### Five-Level Precedence Hierarchy
1. **Level 1 (Pinned):** Agent compliance lock directly serves pinned model.
2. **Level 2 (CacheHit):** Continuation turn returns cached model decision. Observes turn feedback.
3. **Level 2.5 (SmallTalk):** Salience gate detects non-substantive queries (small talk) and serves a cheap model without pinning or classifying.
4. **Level 3 (Classified):** Boundary turn with cache miss:
   1. Executes `request_classifier.classify(ClassifyInput { query, context: None }).await`.
   2. Obtains authoritative `request_type`.
   3. Thompson-samples `Tier` using `classifier::pick_model_thompson(&learned, request_type, DEFAULT_W_QUALITY, DEFAULT_W_COST, &mut rng)`.
   4. Resolves model for provider and tier.
   5. Caches decision `(model, tier, request_type)` for sticky continuation turns.
5. **Level 4 / 5 (Config / Default):** Fallback model.

---

## 5. Summary of Files Changed

- [`llm-router/src/handlers/chat.rs`](file:///d:/Nasiko/repo/llm-router/src/handlers/chat.rs):
  - Passed `ctx.request_classifier.clone()` to `route_model`.
  - Added `request_classifier` to test `ctx_with`.
- [`llm-router/src/handlers/embeddings.rs`](file:///d:/Nasiko/repo/llm-router/src/handlers/embeddings.rs):
  - Added `request_classifier` to test `ctx_with`.
- [`llm-router/src/handlers/responses.rs`](file:///d:/Nasiko/repo/llm-router/src/handlers/responses.rs):
  - Added `request_classifier` to test `ctx`.
- [`llm-router/src/routing/mod.rs`](file:///d:/Nasiko/repo/llm-router/src/routing/mod.rs):
  - Renamed parameter to `request_classifier` to avoid module name shadowing.
  - Used `request_classifier.classify()` output to drive `pick_model_thompson`.
- [`llm-router/src/routing/classifier.rs`](file:///d:/Nasiko/repo/llm-router/src/routing/classifier.rs):
  - Refactored `HostedClassifier::parse_content` and `HostedClassifier::fallback`.
  - Added unit tests for parsing, fences, invalid JSON, bounds clamping, network fallback, and fallback parity.
- [`llm-router/examples/classifier_eval.rs`](file:///d:/Nasiko/repo/llm-router/examples/classifier_eval.rs):
  - Uses shared `RegexClassifier` from routing module.
  - Added fallback path to `classifier-eval.json`.
- [`llm-router/tests/router_e2e.rs`](file:///d:/Nasiko/repo/llm-router/tests/router_e2e.rs):
  - Added integration tests for router using `RequestClassifier`, sticky turn caching, and failing hosted classifier fallback.
