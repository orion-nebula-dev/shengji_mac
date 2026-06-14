#![allow(dead_code)]

use crate::domain::provider::{ProviderCapability, ProviderDescriptor, ProviderLocality};
use crate::providers::AsrProvider;

pub const PROVIDER_ID: &str = "local_whisperkit";
pub const DEFAULT_DISPLAY_NAME: &str = "Local WhisperKit / Argmax";
pub const OPENAI_AUDIO_TRANSCRIPTIONS_PATH: &str = "/v1/audio/transcriptions";
pub const PRIVACY_BOUNDARY: &str =
    "本地 ASR：音频默认留在本机，后续 v0.5 接入 Argmax local server。";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgmaxRuntimeMode {
    LocalServer,
    CliSidecar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalWhisperKitProvider {
    runtime_mode: ArgmaxRuntimeMode,
    transcription_path: &'static str,
}

impl Default for LocalWhisperKitProvider {
    fn default() -> Self {
        Self {
            runtime_mode: ArgmaxRuntimeMode::LocalServer,
            transcription_path: OPENAI_AUDIO_TRANSCRIPTIONS_PATH,
        }
    }
}

impl LocalWhisperKitProvider {
    pub fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        descriptor()
    }

    pub fn runtime_mode(&self) -> &ArgmaxRuntimeMode {
        &self.runtime_mode
    }

    pub fn transcription_path(&self) -> &'static str {
        self.transcription_path
    }
}

impl AsrProvider for LocalWhisperKitProvider {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }
}

pub fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: PROVIDER_ID,
        display_name: DEFAULT_DISPLAY_NAME,
        capability: ProviderCapability::Asr,
        locality: ProviderLocality::Local,
        privacy_boundary: PRIVACY_BOUNDARY,
    }
}
