#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCapability {
    Asr,
    Speaker,
    Semantic,
    Embedding,
    Export,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderLocality {
    Local,
    Cloud,
    Reserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub capability: ProviderCapability,
    pub locality: ProviderLocality,
    pub privacy_boundary: &'static str,
}
