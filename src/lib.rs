//! # Aetheris Cortex
//!
//! Autonomous Model-Agnostic Request Classifier & Adaptive Router.

pub mod classifier;
pub mod patterns;
pub mod routing;

pub use classifier::{
    classify_request_type, Classification, ClassifyInput, HostedClassifier, RegexClassifier,
    RequestClassifier, RequestType,
};
pub use routing::{Phase, RouterState, Tier};
