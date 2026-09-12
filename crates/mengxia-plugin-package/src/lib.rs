//! Plugin package inspection boundary for MengXia.

#![forbid(unsafe_code)]

mod dependency;
mod manifest;

pub use dependency::{RuntimeDependencyDeclaration, RuntimeDependencyRole};
pub use manifest::{
    CapabilityId, InspectedPluginPackage, PackageDigest, PermissionRequest, PluginPackageError,
    inspect_manifest,
};
