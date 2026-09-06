//! MengXia application orchestration boundary.

#![forbid(unsafe_code)]

#[allow(dead_code)]
mod asset_persistence;
mod asset_query;
mod config;
mod ingest;
mod materialize;
mod observability;
mod verification;

pub use asset_query::{
    AssetQueryService, InspectAssetResponse, ListAssetsResponse, opaque_cursor_checksum_is_valid,
};
pub use config::{CoreLogLevel, LibraryConfigDocument, LibraryConfigKey, Task008RuntimeConfig};
pub use observability::{
    CoreAvailability, CoreCorrelationIdentity, CoreLiveness, CoreLogContext, CoreLogEncoder,
    CoreLogEvent, CoreLogEventDraft, CoreLogEventKind, CoreLogSeverity, CoreLogSink,
    CoreLogSinkSnapshot, CoreMetrics, CoreOperationKind, CoreOutcomeKind, CoreReadiness,
    CoreRequestIdentity, CustodyObservation, DbMetricKind, DbMetricOutcome, HealthDegradedClass,
    LibraryHealthInput, LibraryHealthState, LocalSecurityBaseline, MetricCoreOutcome,
    MetricOperation, ObservabilityDropReason, ProviderMetricClass, ReadinessBlockReason, Sensitive,
    StorageDirection, StorageMetricOperation, StorageMetricOutcome,
};
pub use verification::{
    IntegrityIssueResponse, VerificationReportOwner, VerificationRun, VerificationService,
};

pub use asset_persistence::{StartupMutationClassification, StartupMutationClassificationService};
pub use ingest::{
    IngestActivityObservation, IngestAdmissionLimits, IngestAssetCopyRequest,
    IngestAssetCopyResult, IngestAssetCopyService, IngestAssetExecutionError, IngestAssetFailure,
    IngestRetry,
};
pub use materialize::{
    MaterializeAssetFailure, MaterializeAssetRequest, MaterializeAssetResult,
    MaterializeAssetService,
};
