//! MengXia application orchestration boundary.

#![forbid(unsafe_code)]

#[allow(dead_code)]
mod asset_persistence;
mod asset_query;
mod config;
mod ingest;
mod verification;

pub use asset_query::{AssetQueryService, InspectAssetResponse, ListAssetsResponse};
pub use config::{LibraryConfigDocument, LibraryConfigKey};
pub use verification::{IntegrityIssueResponse, VerificationReportOwner, VerificationRun};

pub use ingest::{
    IngestAdmissionLimits, IngestAssetCopyRequest, IngestAssetCopyResult, IngestAssetCopyService,
    IngestAssetExecutionError, IngestAssetFailure, IngestRetry,
};
