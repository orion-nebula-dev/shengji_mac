use crate::domain::provider::{ProviderCapability, ProviderDescriptor, ProviderLocality};

pub mod semantic;

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
        ProviderDescriptor {
            id: "local_whisperkit",
            display_name: "Local WhisperKit / Argmax",
            capability: ProviderCapability::Asr,
            locality: ProviderLocality::Local,
            privacy_boundary: "本地 ASR：音频默认留在本机，后续 v0.5 接入 Argmax local server。",
        },
        ProviderDescriptor {
            id: "local_speakerkit",
            display_name: "Local SpeakerKit",
            capability: ProviderCapability::Speaker,
            locality: ProviderLocality::Local,
            privacy_boundary: "本地说话人分离：仅生成 speaker label 与时间段，不识别真实姓名。",
        },
        ProviderDescriptor {
            id: "minimax_m3",
            display_name: "MiniMax M3",
            capability: ProviderCapability::Semantic,
            locality: ProviderLocality::Cloud,
            privacy_boundary:
                "云端语义理解：仅发送转写后的文本上下文，用于摘要、Todo、脑图和研究。",
        },
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
