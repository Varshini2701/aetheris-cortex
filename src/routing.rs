use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

use crate::classifier::{Classification, ClassifyInput, RequestClassifier, RequestType};

/// Model strength tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tier {
    Tier1, // Flagship (Complex)
    Tier2, // Balanced (Mid)
    Tier3, // Fast & Cheap (Simple)
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Tier1 => "Tier1",
            Tier::Tier2 => "Tier2",
            Tier::Tier3 => "Tier3",
        }
    }
}

/// Routing phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    ColdStart,
    Continue,
    Switch,
}

/// Router state maintaining Level 2 sticky session cache.
#[derive(Default)]
pub struct RouterState {
    l2_cache: Mutex<HashMap<String, (Tier, &'static str)>>,
}

impl RouterState {
    pub fn new() -> Self {
        Self {
            l2_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Route a model turn using the classifier, honoring Level 2 continuation cache.
    pub async fn route(
        &self,
        session_id: &str,
        phase: Phase,
        query: &str,
        context: Option<&str>,
        classifier: &Arc<dyn RequestClassifier>,
    ) -> (Tier, &'static str, bool, Option<Classification>) {
        // Level 2 Continuation Cache Check
        if phase == Phase::Continue {
            let cache = self.l2_cache.lock().unwrap();
            if let Some(&(tier, model)) = cache.get(session_id) {
                // Cache hit: bypass classifier completely (0 overhead)
                return (tier, model, true, None);
            }
        }

        // Fireable boundary (ColdStart or Switch): run single classification pass
        let classification = classifier
            .classify(ClassifyInput {
                query: query.to_string(),
                context: context.map(|s| s.to_string()),
            })
            .await;

        let (tier, model) = match (classification.request_type, classification.complexity) {
            (RequestType::TechnicalDesign | RequestType::AnalyticalReasoning, _)
            | (_, 4..=5) => (Tier::Tier1, "claude-opus-4-8"),

            (RequestType::CodeGeneration | RequestType::CodeUnderstanding, _)
            | (_, 3) => (Tier::Tier2, "claude-sonnet-4-6"),

            _ => (Tier::Tier3, "claude-haiku-4-5"),
        };

        // Populate / update L2 cache
        {
            let mut cache = self.l2_cache.lock().unwrap();
            cache.insert(session_id.to_string(), (tier, model));
        }

        (tier, model, false, Some(classification))
    }
}
