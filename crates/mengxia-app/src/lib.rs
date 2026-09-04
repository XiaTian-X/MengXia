//! MengXia application orchestration boundary.

#![forbid(unsafe_code)]

#[allow(dead_code)]
mod asset_persistence;
mod asset_query;
mod config;
mod ingest;
mod observability;
mod verification;

pub use asset_query::{AssetQueryService, InspectAssetResponse, ListAssetsResponse};
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

pub use ingest::{
    IngestActivityObservation, IngestAdmissionLimits, IngestAssetCopyRequest,
    IngestAssetCopyResult, IngestAssetCopyService, IngestAssetExecutionError, IngestAssetFailure,
    IngestRetry,
};
