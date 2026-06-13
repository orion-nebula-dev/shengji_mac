#![allow(dead_code)]

use crate::domain::provider::{ProviderCapability, ProviderDescriptor, ProviderLocality};
use crate::providers::SpeakerProvider;

pub const PROVIDER_ID: &str = "local_speakerkit";
pub const DEFAULT_DISPLAY_NAME: &str = "Local SpeakerKit";
pub const PRIVACY_BOUNDARY: &str =
    "本地说话人分离：仅生成 speaker label 与时间段，不识别真实姓名。";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeakerKitRuntimeMode {
    LocalServer,
    CliSidecar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSpeakerKitProvider {
    runtime_mode: SpeakerKitRuntimeMode,
    supports_manual_correction: bool,
}

impl Default for LocalSpeakerKitProvider {
    fn default() -> Self {
        Self {
            runtime_mode: SpeakerKitRuntimeMode::LocalServer,
            supports_manual_correction: true,
        }
    }
}

impl LocalSpeakerKitProvider {
    pub fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        descriptor()
    }

    pub fn runtime_mode(&self) -> &SpeakerKitRuntimeMode {
        &self.runtime_mode
    }

    pub fn supports_manual_correction(&self) -> bool {
        self.supports_manual_correction
    }
}

impl SpeakerProvider for LocalSpeakerKitProvider {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }
}

pub fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: PROVIDER_ID,
        display_name: DEFAULT_DISPLAY_NAME,
        capability: ProviderCapability::Speaker,
        locality: ProviderLocality::Local,
        privacy_boundary: PRIVACY_BOUNDARY,
    }
}
