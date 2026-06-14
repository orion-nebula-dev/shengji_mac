#![allow(dead_code)]

use crate::domain::provider::{ProviderCapability, ProviderDescriptor, ProviderLocality};
use crate::providers::ExportProvider;

pub const PROVIDER_ID: &str = "local_file";
pub const DEFAULT_DISPLAY_NAME: &str = "Local File Export";
pub const PRIVACY_BOUNDARY: &str =
    "本地导出：Markdown、JSON、SRT 和分享快照只在本机生成，不上传到外部服务。";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFileExportProvider;

impl LocalFileExportProvider {
    pub fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        descriptor()
    }
}

impl ExportProvider for LocalFileExportProvider {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }
}

pub fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: PROVIDER_ID,
        display_name: DEFAULT_DISPLAY_NAME,
        capability: ProviderCapability::Export,
        locality: ProviderLocality::Local,
        privacy_boundary: PRIVACY_BOUNDARY,
    }
}
