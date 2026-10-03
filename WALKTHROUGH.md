# Walkthrough: P2 Request Classifier Implementation & Validation

## Overview
We completed the implementation of **P2 — Request Classifier** for `nasiko-llm-router`. All architectural requirements, compiler fixes, test suites, and evaluator criteria have been verified and pass cleanly in release mode.

---

## 1. Key Changes Completed

### A. Architectural Fix in `route_model` ([`llm-router/src/routing/mod.rs`](file:///d:/Nasiko/repo/llm-router/src/routing/mod.rs))
- **Eliminated duplicate classification:** The router now invokes the configured `request_classifier` (`Arc<dyn RequestClassifier>`) once at fireable boundaries.
- **Direct Thompson Tier Selection:** Replaced the legacy `classify()` call with `classifier::pick_model_thompson(&learned, request_type, ...)`, directly consuming the classifier's `request_type`.
- **Prevented Parameter Shadowing:** Renamed `classifier` parameter to `request_classifier: Arc<dyn RequestClassifier>` so the `classifier` module is unambiguous.

### B. Updated Call Sites & Context Constructors
- **Fixed `route_model` call in Chat Handler ([`llm-router/src/handlers/chat.rs`](file:///d:/Nasiko/repo/llm-router/src/handlers/chat.rs)):** Passed `ctx.request_classifier.clone()` as the 5th argument.
- **Updated Test Context Constructors:** Added `request_classifier: Arc::new(crate::routing::RegexClassifier)` in:
  - [`llm-router/src/handlers/chat.rs`](file:///d:/Nasiko/repo/llm-router/src/handlers/chat.rs)
  - [`llm-router/src/handlers/embeddings.rs`](file:///d:/Nasiko/repo/llm-router/src/handlers/embeddings.rs)
  - [`llm-router/src/handlers/responses.rs`](file:///d:/Nasiko/repo/llm-router/src/handlers/responses.rs)

### C. HostedClassifier Robustness & Parsing ([`llm-router/src/routing/classifier.rs`](file:///d:/Nasiko/repo/llm-router/src/routing/classifier.rs))
- Exposed `HostedClassifier::parse_content(content: &str, query: &str) -> Classification` and `HostedClassifier::fallback(query: &str) -> Classification`.
- Handles raw JSON and markdown code block fences (````json ... ```` or ```` ... ````).
- Enforces bounds: complexity clamped to `1..=5`, confidence clamped to `0.0..=1.0`.
- Deterministic fallback to `RegexClassifier` on invalid JSON, unknown request types, non-finite confidence, HTTP errors, and network timeouts.

### D. Evaluator Binary ([`llm-router/examples/classifier_eval.rs`](file:///d:/Nasiko/repo/llm-router/examples/classifier_eval.rs))
- Evaluator loads classifier backend once before the loop.
- Measures classification latency (`latency_us`) strictly around inference/classification.
- Streams output to JSONL with `id`, `request_type`, `complexity`, `confidence`, and `latency_us`.
- Configurable via environment variables (`CLASSIFIER_BACKEND`, `CLASSIFIER_MODEL`, `CLASSIFIER_ENDPOINT`, `CLASSIFIER_TIMEOUT_SECS`, `GROQ_API_KEY`).

---

## 2. Test Execution & Verification

### Compilation & Formatting
```cmd
cargo fmt -p nasiko-llm-router
cargo check -p nasiko-llm-router --release
```
**Result:** Build succeeded with 0 errors and 0 warnings.

### Full Test Suite
```cmd
cargo test -p nasiko-llm-router --release
```

#### Unit Tests in `nasiko-llm-router`:
```text
test routing::classifier::tests::regex_request_classifier_uses_existing_classifier ... ok
test routing::classifier::tests::hosted_response_parsing_valid_json ... ok
test routing::classifier::tests::hosted_response_parsing_markdown_fence ... ok
test routing::classifier::tests::invalid_hosted_response_fallback ... ok
test routing::classifier::tests::complexity_bounds_clamped ... ok
test routing::classifier::tests::confidence_bounds_clamped ... ok
test routing::classifier::tests::hosted_network_failure_fallback ... ok
test routing::classifier::tests::classifier_fallback_behavior_matches_regex ... ok
...
test result: ok. 388 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 30.04s
```

#### Integration & E2E Tests in `tests/router_e2e.rs`:
```text
test only_free_flowing_boundaries_fire ... ok
test phase_and_mode_parse_safely ... ok
test routing_using_request_classifier ... ok
test tool_loop_stays_sticky ... ok
test sticky_continuation_does_not_reclassify ... ok
test requests_classify_to_expected_types ... ok
test same_seed_same_tier ... ok
test router_fallback_when_hosted_classifier_fails ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

---

## 3. Running the Classifier Evaluator

### Regex Baseline (Default)
In Windows CMD:
```cmd
cargo run --release -p nasiko-llm-router --example classifier_eval
```
Or with explicit paths:
```cmd
set EVAL_SET=classifier-eval.json
set OUT=classifier-out.jsonl
cargo run --release -p nasiko-llm-router --example classifier_eval
```

### Groq / Hosted Model Evaluation
In Windows CMD:
```cmd
set CLASSIFIER_BACKEND=groq
set CLASSIFIER_MODEL=openai/gpt-oss-20b
set CLASSIFIER_ENDPOINT=https://api.groq.com/openai/v1/chat/completions
set CLASSIFIER_TIMEOUT_SECS=5
set GROQ_API_KEY=your_actual_groq_api_key_here
set EVAL_SET=classifier-eval.json
set OUT=classifier-out.jsonl
cargo run --release -p nasiko-llm-router --example classifier_eval
```

Output format in `classifier-out.jsonl`:
```json
{"complexity":1,"confidence":0.99,"id":"pub-01","latency_us":521045,"request_type":"code_generation"}
{"complexity":1,"confidence":0.99,"id":"pub-02","latency_us":312019,"request_type":"factual_lookup"}
```
