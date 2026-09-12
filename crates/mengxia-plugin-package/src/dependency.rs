use mengxia_types::Sha256Digest;

/// The only execution roles declared by the V1 manifest.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeDependencyRole {
    PluginEntrypoint,
    Tool,
}

impl RuntimeDependencyRole {
    pub(crate) const fn as_manifest_str(self) -> &'static str {
        match self {
            Self::PluginEntrypoint => "PLUGIN_ENTRYPOINT",
            Self::Tool => "TOOL",
        }
    }
}

/// Immutable logical executable identity. It deliberately contains no path or command.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDependencyDeclaration {
    pub(crate) dependency_id: Box<str>,
    pub(crate) role: RuntimeDependencyRole,
    pub(crate) target: Box<str>,
    pub(crate) byte_length: u64,
    pub(crate) sha256: Sha256Digest,
}

impl RuntimeDependencyDeclaration {
    #[must_use]
    pub fn dependency_id(&self) -> &str {
        &self.dependency_id
    }

    #[must_use]
    pub const fn role(&self) -> RuntimeDependencyRole {
        self.role
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[must_use]
    pub const fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
}
