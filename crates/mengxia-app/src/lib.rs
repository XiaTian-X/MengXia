//! MengXia application orchestration boundary.

#![forbid(unsafe_code)]

#[allow(dead_code)]
mod asset_persistence;
mod config;
mod ingest;

pub use config::{LibraryConfigDocument, LibraryConfigKey};

pub use ingest::{
    IngestAdmissionLimits, IngestAssetCopyRequest, IngestAssetCopyResult, IngestAssetCopyService,
    IngestAssetExecutionError, IngestAssetFailure, IngestRetry,
};
