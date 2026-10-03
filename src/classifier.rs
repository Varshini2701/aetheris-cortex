use std::time::Duration;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::patterns::CATEGORY_PATTERNS;

/// The coarse kind of work a query represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestType {
    CodeGeneration,
    CodeUnderstanding,
    TechnicalDesign,
    AnalyticalReasoning,
    Writing,
    FactualLookup,
    General,
}

impl RequestType {
    pub fn as_str(self) -> &'static str {
        match self {
            RequestType::CodeGeneration => "code_generation",
            RequestType::CodeUnderstanding => "code_understanding",
            RequestType::TechnicalDesign => "technical_design",
            RequestType::AnalyticalReasoning => "analytical_reasoning",
            RequestType::Writing => "writing",
            RequestType::FactualLookup => "factual_lookup",
            RequestType::General => "general",
        }
    }

    pub fn from_wire(s: &str) -> Option<RequestType> {
        Some(match s {
            "code_generation" => RequestType::CodeGeneration,
            "code_understanding" => RequestType::CodeUnderstanding,
            "technical_design" => RequestType::TechnicalDesign,
            "analytical_reasoning" => RequestType::AnalyticalReasoning,
            "writing" => RequestType::Writing,
            "factual_lookup" => RequestType::FactualLookup,
            "general" => RequestType::General,
            _ => return None,
        })
    }
}

impl std::fmt::Display for RequestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Input to a model-backed request classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyInput {
    pub query: String,
    pub context: Option<String>,
}

/// Structured classification output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub request_type: RequestType,
    pub complexity: u8,
    pub confidence: f32,
    pub fallback: bool,
}

/// Model-agnostic interface for request classification.
#[async_trait]
pub trait RequestClassifier: Send + Sync {
    async fn classify(&self, input: ClassifyInput) -> Classification;
}

/// Fast regex vote-count classifier.
pub struct RegexClassifier;

#[async_trait]
impl RequestClassifier for RegexClassifier {
    async fn classify(&self, input: ClassifyInput) -> Classification {
        Classification {
            request_type: classify_request_type(&input.query),
            complexity: 3,
            confidence: 0.0,
            fallback: false,
        }
    }
}

/// Bucket a query into a [`RequestType`] by regex vote count.
pub fn classify_request_type(text: &str) -> RequestType {
    let mut best = RequestType::General;
    let mut best_score = 0usize;

    for (rt, pats) in CATEGORY_PATTERNS.iter() {
        let score = pats.iter().filter(|p| p.is_match(text)).count();
        if score > best_score {
            best_score = score;
            best = *rt;
        }
    }

    best
}

/// Hosted model classifier (OpenAI/Groq compatible).
pub struct HostedClassifier {
    client: Client,
    endpoint: String,
    model: String,
    api_key: String,
}

impl HostedClassifier {
    pub fn new(
        endpoint: String,
        model: String,
        api_key: String,
        timeout: Duration,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self {
            client,
            endpoint,
            model,
            api_key,
        })
    }

    pub fn fallback(query: &str) -> Classification {
        Classification {
            request_type: classify_request_type(query),
            complexity: 3,
            confidence: 0.0,
            fallback: true,
        }
    }

    pub fn parse_content(content: &str, query: &str) -> Classification {
        let trimmed = content.trim();
        let json_content = trimmed
            .strip_prefix("```json")
            .and_then(|s| s.strip_suffix("```"))
            .or_else(|| {
                trimmed
                    .strip_prefix("```")
                    .and_then(|s| s.strip_suffix("```"))
            })
            .map(str::trim)
            .unwrap_or(trimmed);

        let parsed: ModelClassification = match serde_json::from_str(json_content) {
            Ok(parsed) => parsed,
            Err(_) => return Self::fallback(query),
        };

        if !parsed.confidence.is_finite() {
            return Self::fallback(query);
        }

        let request_type = match RequestType::from_wire(&parsed.request_type) {
            Some(rt) => rt,
            None => return Self::fallback(query),
        };

        Classification {
            request_type,
            complexity: parsed.complexity.clamp(1, 5),
            confidence: parsed.confidence.clamp(0.0, 1.0),
            fallback: false,
        }
    }
}

#[derive(Debug, Deserialize)]
struct HostedResponse {
    choices: Vec<HostedChoice>,
}

#[derive(Debug, Deserialize)]
struct HostedChoice {
    message: HostedMessage,
}

#[derive(Debug, Deserialize)]
struct HostedMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ModelClassification {
    request_type: String,
    complexity: u8,
    confidence: f32,
}

#[async_trait]
impl RequestClassifier for HostedClassifier {
    async fn classify(&self, input: ClassifyInput) -> Classification {
        let system = r#"You are a deterministic request classifier for an LLM router.
Classify the user's PRIMARY task into exactly one request_type:
- code_generation
- code_understanding
- technical_design
- analytical_reasoning
- writing
- factual_lookup
- general

Return ONLY a JSON object:
{
  "request_type": "...",
  "complexity": 1,
  "confidence": 0.0
}"#;

        let user = match input.context.as_deref() {
            Some(context) if !context.is_empty() => {
                format!("Context:\n{}\n\nUser request:\n{}", context, input.query)
            }
            _ => format!("User request:\n{}", input.query),
        };

        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });

        let response = match self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(_) => return Self::fallback(&input.query),
        };

        if !response.status().is_success() {
            return Self::fallback(&input.query);
        }

        let payload: HostedResponse = match response.json().await {
            Ok(payload) => payload,
            Err(_) => return Self::fallback(&input.query),
        };

        let content = match payload.choices.first() {
            Some(choice) => choice.message.content.as_str(),
            None => return Self::fallback(&input.query),
        };

        Self::parse_content(content, &input.query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_clamps_and_parses() {
        let valid = r#"{"request_type": "code_generation", "complexity": 9, "confidence": 1.4}"#;
        let c = HostedClassifier::parse_content(valid, "query");
        assert_eq!(c.request_type, RequestType::CodeGeneration);
        assert_eq!(c.complexity, 5);
        assert_eq!(c.confidence, 1.0);
        assert!(!c.fallback);
    }

    #[test]
    fn parse_content_falls_back_on_malformed_json() {
        let bad = "Not JSON";
        let c = HostedClassifier::parse_content(bad, "write a function in rust");
        assert_eq!(c.request_type, RequestType::CodeGeneration);
        assert!(c.fallback);
    }

    #[tokio::test]
    async fn regex_classifier_works() {
        let classifier = RegexClassifier;
        let c = classifier
            .classify(ClassifyInput {
                query: "write an algorithm to sort numbers".into(),
                context: None,
            })
            .await;
        assert_eq!(c.request_type, RequestType::CodeGeneration);
    }
}
