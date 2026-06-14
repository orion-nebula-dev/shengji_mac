use crate::domain::provider::{ProviderCapability, ProviderDescriptor, ProviderLocality};

pub mod asr;
pub mod semantic;
pub mod speaker;

#[allow(dead_code)]
pub trait AsrProvider {
    fn provider_id(&self) -> &'static str;
}

#[allow(dead_code)]
pub trait SpeakerProvider {
    fn provider_id(&self) -> &'static str;
}

#[allow(dead_code)]
pub trait SemanticProvider {
    fn provider_id(&self) -> &'static str;
}

#[allow(dead_code)]
pub trait EmbeddingProvider {
    fn provider_id(&self) -> &'static str;
}

#[allow(dead_code)]
pub trait ExportProvider {
    fn provider_id(&self) -> &'static str;
}

pub fn provider_catalog() -> Vec<ProviderDescriptor> {
    vec![
        asr::local_whisperkit::descriptor(),
        speaker::local_speakerkit::descriptor(),
        semantic::minimax_m3::descriptor(),
        ProviderDescriptor {
            id: "reserved",
            display_name: "Reserved Embedding Provider",
            capability: ProviderCapability::Embedding,
            locality: ProviderLocality::Reserved,
            privacy_boundary: "Embedding 预留：v0.4 不启用默认路径，不发送任何用户数据。",
        },
        ProviderDescriptor {
            id: "local_file",
            display_name: "Local File Export",
            capability: ProviderCapability::Export,
            locality: ProviderLocality::Local,
            privacy_boundary: "本地导出：Markdown、JSON、SRT 等导出默认写入用户选择的位置。",
        },
    ]
}
