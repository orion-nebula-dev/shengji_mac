#![allow(dead_code)]

use crate::domain::provider::{ProviderCapability, ProviderDescriptor, ProviderLocality};
use crate::providers::SemanticProvider;

pub const PROVIDER_ID: &str = "minimax_m3";
pub const DEFAULT_DISPLAY_NAME: &str = "MiniMax M3";
pub const DEFAULT_BASE_URL: &str = "https://api.minimax.io/v1/responses";
pub const DEFAULT_MODEL_NAME: &str = "MiniMax-M3";
pub const MAX_CONTEXT_TOKENS: usize = 1_000_000;
pub const PRIVACY_BOUNDARY: &str =
    "云端语义理解：仅发送转写后的文本上下文，用于摘要、Todo、脑图和研究。";
pub const SUPPORTED_ARTIFACT_TYPES: &[&str] = &[
    "transcript_revision",
    "recording_type",
    "summary",
    "meeting_minutes",
    "todo_extraction",
    "mind_map",
    "moment",
    "deep_research",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniMaxM3Provider {
    base_url: &'static str,
    model_name: &'static str,
}

impl Default for MiniMaxM3Provider {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL,
            model_name: DEFAULT_MODEL_NAME,
        }
    }
}

impl MiniMaxM3Provider {
    pub fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        descriptor()
    }

    pub fn base_url(&self) -> &'static str {
        self.base_url
    }

    pub fn model_name(&self) -> &'static str {
        self.model_name
    }

    pub fn supports_artifact_type(&self, artifact_type: &str) -> bool {
        SUPPORTED_ARTIFACT_TYPES.contains(&artifact_type)
    }
}

impl SemanticProvider for MiniMaxM3Provider {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }
}

pub fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: PROVIDER_ID,
        display_name: DEFAULT_DISPLAY_NAME,
        capability: ProviderCapability::Semantic,
        locality: ProviderLocality::Cloud,
        privacy_boundary: PRIVACY_BOUNDARY,
    }
}
