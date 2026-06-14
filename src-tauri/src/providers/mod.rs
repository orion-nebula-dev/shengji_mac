use crate::domain::provider::{ProviderCapability, ProviderDescriptor, ProviderLocality};

pub mod asr;
pub mod export;
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
        export::local_file::descriptor(),
    ]
}
