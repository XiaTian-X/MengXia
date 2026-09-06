use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use mengxia_ports::{
    AssetStoreError, IngestControl, IngestDirective, IngestStop, IntegrityFinding, IntegrityIssue,
    IntegrityIssueKind, IntegrityIssuePage, IntegrityObjectId, IntegrityObjectKind,
    IntegrityRemediation, IntegritySeverity, InterruptibleSqliteControl,
    RegisteredBlobVerificationPort, VerificationMode, VerificationReportIdentity,
    VerificationScanPosition, VerificationStorePort, VerificationSummary,
};
use mengxia_types::{Id, IdGenerationError};
use sha2::{Digest as _, Sha256};

const COMPLETED_REPORTS_MAX: usize = 4;
const STORED_ISSUES_MAX: usize = 4096;
const ID_ATTEMPTS_MAX: usize = 8;
const ISSUE_CURSOR_LENGTH: usize = 96;
const ISSUE_CURSOR_PREFIX_LENGTH: usize = 64;
const ISSUE_CURSOR_MAGIC: [u8; 8] = *b"MXVCUR1\0";
const CURSOR_FORMAT_VERSION: u16 = 1;
const ISSUE_OPERATION_DISCRIMINATOR: u32 = 3;

trait VerificationIdentitySource: Send + Sync {
    fn next_id(&self) -> Result<Id<VerificationReportIdentity>, IdGenerationError>;
}

struct SystemVerificationIdentitySource;

impl VerificationIdentitySource for SystemVerificationIdentitySource {
    fn next_id(&self) -> Result<Id<VerificationReportIdentity>, IdGenerationError> {
        Id::try_new()
    }
}

struct CompletedReport {
    summary: VerificationSummary,
    issues: Vec<IntegrityIssue>,
}

struct ReportState {
    active: bool,
    reports: VecDeque<Arc<CompletedReport>>,
}

struct ReportInner {
    identity_source: Arc<dyn VerificationIdentitySource>,
    state: Mutex<ReportState>,
}

/// Owns the single active verification and four immutable completed reports.
#[derive(Clone)]
pub struct VerificationReportOwner {
    inner: Arc<ReportInner>,
    library_id: [u8; 16],
}

pub struct IntegrityIssueResponse {
    page: IntegrityIssuePage,
    next_cursor: Option<[u8; ISSUE_CURSOR_LENGTH]>,
}

impl IntegrityIssueResponse {
    #[must_use]
    pub const fn page(&self) -> &IntegrityIssuePage {
        &self.page
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&[u8; ISSUE_CURSOR_LENGTH]> {
        self.next_cursor.as_ref()
    }
}

pub struct VerificationRun {
    inner: Arc<ReportInner>,
    verification_id: Id<VerificationReportIdentity>,
    mode: VerificationMode,
    snapshot_commit_sequence: Option<u64>,
    discovered_issue_count: u64,
    issues: Vec<IntegrityIssue>,
    has_fatal_local_issue: bool,
    has_custody_degradation: bool,
    first_fatal_issue: Option<IntegrityIssue>,
    finished: bool,
}

pub struct VerificationService<S, P> {
    store: Arc<S>,
    physical: Arc<P>,
    reports: VerificationReportOwner,
}

impl<S, P> VerificationService<S, P>
where
    S: VerificationStorePort,
    P: RegisteredBlobVerificationPort,
{
    #[must_use]
    pub fn new(store: Arc<S>, physical: Arc<P>, reports: VerificationReportOwner) -> Self {
        Self {
            store,
            physical,
            reports,
        }
    }

    pub async fn verify(
        &self,
        mode: VerificationMode,
    ) -> Result<VerificationSummary, AssetStoreError> {
        self.verify_controlled(mode, Arc::new(ContinueVerification))
            .await
    }

    pub async fn verify_controlled(
        &self,
        mode: VerificationMode,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<VerificationSummary, AssetStoreError> {
        checkpoint(&control)?;
        let mut report = self.reports.begin(mode)?;
        let snapshot = self
            .store
            .capture_verification_snapshot(Arc::clone(&control))
            .await?;
        report.bind_snapshot(snapshot.snapshot_commit_sequence())?;
        let mut position = VerificationScanPosition::CommandsAfter(None);
        loop {
            checkpoint(&control)?;
            let page = self
                .store
                .scan_verification_page(snapshot, position, Arc::clone(&control))
                .await?;
            let (findings, candidates, next) = page.__into_app();
            for finding in findings {
                checkpoint(&control)?;
                report.record_finding(finding)?;
            }
            for candidate in candidates {
                checkpoint(&control)?;
                if let Some(finding) = self
                    .physical
                    .verify_registered_blob(
                        candidate,
                        mode,
                        Arc::clone(&control) as Arc<dyn IngestControl>,
                    )
                    .await?
                {
                    report.record_finding(finding)?;
                }
            }
            if next == VerificationScanPosition::Complete {
                return report.finish(false);
            }
            if next == position {
                return Err(AssetStoreError::Internal);
            }
            position = next;
        }
    }

    pub fn list_issues(
        &self,
        verification_id: Id<VerificationReportIdentity>,
        page_size: u32,
        cursor: Option<&[u8]>,
    ) -> Result<IntegrityIssueResponse, AssetStoreError> {
        self.reports.list_issues(verification_id, page_size, cursor)
    }
}

struct ContinueVerification;

impl IngestControl for ContinueVerification {
    fn checkpoint(&self) -> IngestDirective {
        IngestDirective::Continue
    }
}

impl InterruptibleSqliteControl for ContinueVerification {
    fn register_interrupt(
        &self,
        _interrupt: Box<dyn mengxia_ports::SqliteInterrupt>,
    ) -> Result<IngestDirective, mengxia_ports::SqliteInterruptControlError> {
        Ok(IngestDirective::Continue)
    }

    fn clear_interrupt(&self) -> Result<(), mengxia_ports::SqliteInterruptControlError> {
        Ok(())
    }
}

fn checkpoint(control: &Arc<dyn InterruptibleSqliteControl>) -> Result<(), AssetStoreError> {
    match control.checkpoint() {
        IngestDirective::Continue => Ok(()),
        IngestDirective::Stop(IngestStop::Cancelled) => Err(AssetStoreError::OperationCancelled),
        IngestDirective::Stop(IngestStop::DeadlineReached) => {
            Err(AssetStoreError::DeadlineExceeded)
        }
    }
}

impl VerificationReportOwner {
    pub fn new(library_id: [u8; 16]) -> Result<Self, AssetStoreError> {
        Self::with_identity_source(library_id, Arc::new(SystemVerificationIdentitySource))
    }

    fn with_identity_source(
        library_id: [u8; 16],
        identity_source: Arc<dyn VerificationIdentitySource>,
    ) -> Result<Self, AssetStoreError> {
        if library_id == [0; 16] {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self {
            inner: Arc::new(ReportInner {
                identity_source,
                state: Mutex::new(ReportState {
                    active: false,
                    reports: VecDeque::with_capacity(COMPLETED_REPORTS_MAX),
                }),
            }),
            library_id,
        })
    }

    pub fn begin(&self, mode: VerificationMode) -> Result<VerificationRun, AssetStoreError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| AssetStoreError::Internal)?;
        if state.active {
            return Err(AssetStoreError::Backpressure);
        }
        let mut verification_id = None;
        for _ in 0..ID_ATTEMPTS_MAX {
            let candidate = self
                .inner
                .identity_source
                .next_id()
                .map_err(|_| AssetStoreError::IdGenerationUnavailable)?;
            if !state
                .reports
                .iter()
                .any(|report| report.summary.verification_id() == candidate)
            {
                verification_id = Some(candidate);
                break;
            }
        }
        let verification_id = verification_id.ok_or(AssetStoreError::IdGenerationUnavailable)?;
        state.active = true;
        drop(state);
        Ok(VerificationRun {
            inner: Arc::clone(&self.inner),
            verification_id,
            mode,
            snapshot_commit_sequence: None,
            discovered_issue_count: 0,
            issues: Vec::with_capacity(STORED_ISSUES_MAX),
            has_fatal_local_issue: false,
            has_custody_degradation: false,
            first_fatal_issue: None,
            finished: false,
        })
    }

    pub fn list_issues(
        &self,
        verification_id: Id<VerificationReportIdentity>,
        page_size: u32,
        cursor: Option<&[u8]>,
    ) -> Result<IntegrityIssueResponse, AssetStoreError> {
        let page_size = usize::try_from(page_size).map_err(|_| AssetStoreError::Validation)?;
        if !(1..=64).contains(&page_size) {
            return Err(AssetStoreError::Validation);
        }
        let has_cursor = !matches!(cursor, None | Some([]));
        let next_ordinal = match cursor {
            None | Some([]) => 1,
            Some(cursor) => decode_issue_cursor(cursor, self.library_id, verification_id)?,
        };
        let report = {
            let state = self
                .inner
                .state
                .lock()
                .map_err(|_| AssetStoreError::Internal)?;
            state
                .reports
                .iter()
                .find(|report| report.summary.verification_id() == verification_id)
                .cloned()
                .ok_or(AssetStoreError::NotFound)?
        };
        let start = usize::try_from(next_ordinal - 1).map_err(|_| AssetStoreError::Validation)?;
        if start > report.issues.len() || (has_cursor && start == report.issues.len()) {
            return Err(AssetStoreError::Validation);
        }
        let end = start.saturating_add(page_size).min(report.issues.len());
        let issues = report.issues[start..end].to_vec();
        let next_ordinal = if end < report.issues.len() {
            Some(u32::try_from(end + 1).map_err(|_| AssetStoreError::Internal)?)
        } else {
            None
        };
        let page = IntegrityIssuePage::__from_app(
            verification_id,
            issues,
            next_ordinal,
            report.summary.discovered_issue_count(),
            report.summary.stored_issue_count(),
            report.summary.dropped_issue_count(),
        )?;
        let next_cursor = next_ordinal
            .map(|ordinal| encode_issue_cursor(self.library_id, verification_id, ordinal))
            .transpose()?;
        Ok(IntegrityIssueResponse { page, next_cursor })
    }
}

impl VerificationRun {
    pub fn bind_snapshot(&mut self, snapshot_commit_sequence: u64) -> Result<(), AssetStoreError> {
        if self
            .snapshot_commit_sequence
            .replace(snapshot_commit_sequence)
            .is_some()
        {
            return Err(AssetStoreError::Internal);
        }
        Ok(())
    }

    pub fn record_issue(
        &mut self,
        kind: IntegrityIssueKind,
        severity: IntegritySeverity,
        object_kind: IntegrityObjectKind,
        object_id: Option<IntegrityObjectId>,
        remediation: IntegrityRemediation,
    ) -> Result<(), AssetStoreError> {
        self.discovered_issue_count = self
            .discovered_issue_count
            .checked_add(1)
            .ok_or(AssetStoreError::Internal)?;
        let ordinal =
            u32::try_from(self.discovered_issue_count).map_err(|_| AssetStoreError::Internal)?;
        let issue =
            IntegrityIssue::new(ordinal, kind, severity, object_kind, object_id, remediation)?;
        self.has_custody_degradation |= severity == IntegritySeverity::DegradedCustody;
        if severity == IntegritySeverity::FatalLocal {
            self.has_fatal_local_issue = true;
            self.first_fatal_issue.get_or_insert(issue);
        }
        if self.issues.len() < STORED_ISSUES_MAX {
            self.issues.push(issue);
        }
        Ok(())
    }

    pub fn record_finding(&mut self, finding: IntegrityFinding) -> Result<(), AssetStoreError> {
        self.record_issue(
            finding.kind(),
            finding.severity(),
            finding.object_kind(),
            finding.object_id(),
            finding.remediation(),
        )
    }

    pub fn finish(
        mut self,
        canonical_extra_classification_deferred: bool,
    ) -> Result<VerificationSummary, AssetStoreError> {
        let stored_issue_count =
            u32::try_from(self.issues.len()).map_err(|_| AssetStoreError::Internal)?;
        let dropped_issue_count = self
            .discovered_issue_count
            .checked_sub(u64::from(stored_issue_count))
            .ok_or(AssetStoreError::Internal)?;
        let summary = VerificationSummary::__from_app(
            self.verification_id,
            self.mode,
            self.snapshot_commit_sequence
                .ok_or(AssetStoreError::Internal)?,
            self.discovered_issue_count,
            stored_issue_count,
            dropped_issue_count,
            self.has_fatal_local_issue,
            self.has_custody_degradation,
            canonical_extra_classification_deferred,
            self.first_fatal_issue,
        )?;
        let report = Arc::new(CompletedReport {
            summary,
            issues: std::mem::take(&mut self.issues),
        });
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| AssetStoreError::Internal)?;
        if !state.active {
            return Err(AssetStoreError::Internal);
        }
        if state.reports.len() == COMPLETED_REPORTS_MAX {
            state.reports.pop_front();
        }
        state.reports.push_back(report);
        state.active = false;
        self.finished = true;
        Ok(summary)
    }
}

impl Drop for VerificationRun {
    fn drop(&mut self) {
        if !self.finished
            && let Ok(mut state) = self.inner.state.lock()
        {
            state.active = false;
        }
    }
}

fn decode_issue_cursor(
    cursor: &[u8],
    expected_library_id: [u8; 16],
    expected_verification_id: Id<VerificationReportIdentity>,
) -> Result<u32, AssetStoreError> {
    let cursor: &[u8; ISSUE_CURSOR_LENGTH] =
        cursor.try_into().map_err(|_| AssetStoreError::Validation)?;
    if cursor[..8] != ISSUE_CURSOR_MAGIC
        || u16::from_be_bytes(cursor[8..10].try_into().expect("fixed cursor range"))
            != CURSOR_FORMAT_VERSION
        || usize::from(u16::from_be_bytes(
            cursor[10..12].try_into().expect("fixed cursor range"),
        )) != ISSUE_CURSOR_LENGTH
        || u32::from_be_bytes(cursor[12..16].try_into().expect("fixed cursor range"))
            != ISSUE_OPERATION_DISCRIMINATOR
        || cursor[16..32] != expected_library_id
        || cursor[32..48] != expected_verification_id.to_bytes()
        || cursor[52..64].iter().any(|byte| *byte != 0)
        || Sha256::digest(&cursor[..ISSUE_CURSOR_PREFIX_LENGTH]).as_slice()
            != &cursor[ISSUE_CURSOR_PREFIX_LENGTH..]
    {
        return Err(AssetStoreError::Validation);
    }
    let ordinal = u32::from_be_bytes(cursor[48..52].try_into().expect("fixed cursor range"));
    if ordinal == 0 {
        return Err(AssetStoreError::Validation);
    }
    Ok(ordinal)
}

fn encode_issue_cursor(
    library_id: [u8; 16],
    verification_id: Id<VerificationReportIdentity>,
    next_ordinal: u32,
) -> Result<[u8; ISSUE_CURSOR_LENGTH], AssetStoreError> {
    if library_id == [0; 16] || next_ordinal == 0 {
        return Err(AssetStoreError::Internal);
    }
    let mut cursor = [0_u8; ISSUE_CURSOR_LENGTH];
    cursor[..8].copy_from_slice(&ISSUE_CURSOR_MAGIC);
    cursor[8..10].copy_from_slice(&CURSOR_FORMAT_VERSION.to_be_bytes());
    cursor[10..12].copy_from_slice(&(ISSUE_CURSOR_LENGTH as u16).to_be_bytes());
    cursor[12..16].copy_from_slice(&ISSUE_OPERATION_DISCRIMINATOR.to_be_bytes());
    cursor[16..32].copy_from_slice(&library_id);
    cursor[32..48].copy_from_slice(&verification_id.to_bytes());
    cursor[48..52].copy_from_slice(&next_ordinal.to_be_bytes());
    let checksum: [u8; 32] = Sha256::digest(&cursor[..ISSUE_CURSOR_PREFIX_LENGTH]).into();
    cursor[ISSUE_CURSOR_PREFIX_LENGTH..].copy_from_slice(&checksum);
    Ok(cursor)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use super::*;

    struct FixedIds {
        next: AtomicU8,
    }

    struct FakeStore {
        scans: AtomicUsize,
    }

    impl VerificationStorePort for FakeStore {
        fn capture_verification_snapshot(
            &self,
            _control: Arc<dyn InterruptibleSqliteControl>,
        ) -> mengxia_ports::AssetPortFuture<'_, mengxia_ports::VerificationSnapshot> {
            Box::pin(async { mengxia_ports::VerificationSnapshot::__from_store([0x11; 16], 9) })
        }

        fn scan_verification_page(
            &self,
            _snapshot: mengxia_ports::VerificationSnapshot,
            position: VerificationScanPosition,
            _control: Arc<dyn InterruptibleSqliteControl>,
        ) -> mengxia_ports::AssetPortFuture<'_, mengxia_ports::VerificationStorePage> {
            self.scans.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move {
                match position {
                    VerificationScanPosition::CommandsAfter(None) => {
                        mengxia_ports::VerificationStorePage::__from_store(
                            vec![IntegrityFinding::new(
                                IntegrityIssueKind::StagingOrphan,
                                IntegritySeverity::OperatorAction,
                                IntegrityObjectKind::Staging,
                                None,
                                IntegrityRemediation::FutureAdminAction,
                            )?],
                            Vec::new(),
                            VerificationScanPosition::ManagedLocationsAfter(None),
                        )
                    }
                    VerificationScanPosition::ManagedLocationsAfter(None) => {
                        mengxia_ports::VerificationStorePage::__from_store(
                            Vec::new(),
                            Vec::new(),
                            VerificationScanPosition::Complete,
                        )
                    }
                    _ => Err(AssetStoreError::Internal),
                }
            })
        }
    }

    struct FakePhysical;

    struct AlreadyCancelled;

    impl IngestControl for AlreadyCancelled {
        fn checkpoint(&self) -> IngestDirective {
            IngestDirective::Stop(IngestStop::Cancelled)
        }
    }

    impl InterruptibleSqliteControl for AlreadyCancelled {
        fn register_interrupt(
            &self,
            _interrupt: Box<dyn mengxia_ports::SqliteInterrupt>,
        ) -> Result<IngestDirective, mengxia_ports::SqliteInterruptControlError> {
            Ok(IngestDirective::Stop(IngestStop::Cancelled))
        }

        fn clear_interrupt(&self) -> Result<(), mengxia_ports::SqliteInterruptControlError> {
            Err(mengxia_ports::SqliteInterruptControlError::RegistrationConflict)
        }
    }

    impl RegisteredBlobVerificationPort for FakePhysical {
        fn verify_registered_blob(
            &self,
            _candidate: mengxia_ports::RegisteredBlobVerificationCandidate,
            _mode: VerificationMode,
            _control: Arc<dyn IngestControl>,
        ) -> mengxia_ports::AssetPortFuture<'_, Option<IntegrityFinding>> {
            Box::pin(async { Ok(None) })
        }
    }

    impl VerificationIdentitySource for FixedIds {
        fn next_id(&self) -> Result<Id<VerificationReportIdentity>, IdGenerationError> {
            let tail = self.next.fetch_add(1, Ordering::Relaxed);
            let mut bytes = [
                0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
                0x77, 0,
            ];
            bytes[15] = tail;
            Id::from_bytes(bytes).map_err(|_| IdGenerationError::EntropyUnavailable)
        }
    }

    fn owner() -> VerificationReportOwner {
        VerificationReportOwner::with_identity_source(
            [0x11; 16],
            Arc::new(FixedIds {
                next: AtomicU8::new(1),
            }),
        )
        .unwrap()
    }

    fn record_operator_issue(run: &mut VerificationRun) {
        run.record_issue(
            IntegrityIssueKind::StagingOrphan,
            IntegritySeverity::OperatorAction,
            IntegrityObjectKind::Staging,
            None,
            IntegrityRemediation::FutureAdminAction,
        )
        .unwrap();
    }

    fn block_on_ready<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }

    #[test]
    fn active_limit_report_cap_issue_cap_and_cursor_are_exact() {
        let owner = owner();
        let mut active = owner.begin(VerificationMode::Normal).unwrap();
        assert!(matches!(
            owner.begin(VerificationMode::Deep),
            Err(AssetStoreError::Backpressure)
        ));
        active.bind_snapshot(7).unwrap();
        for _ in 0..=STORED_ISSUES_MAX {
            record_operator_issue(&mut active);
        }
        let summary = active.finish(false).unwrap();
        assert_eq!(summary.discovered_issue_count(), 4097);
        assert_eq!(summary.stored_issue_count(), 4096);
        assert_eq!(summary.dropped_issue_count(), 1);

        let first = owner
            .list_issues(summary.verification_id(), 1, None)
            .unwrap();
        assert_eq!(first.page().issues()[0].ordinal(), 1);
        let first_cursor = first.next_cursor().unwrap();
        assert_eq!(
            first_cursor
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "4d58564355523100000100600000000311111111111111111111111111111111018d442fc0007a11802233445566770100000002000000000000000000000000681d07e74ec863e1a3b4dac2b5e51e6e2d3baeccb4035dbdd01b826d65e6f85a"
        );
        for index in [0, 8, 10, 12, 16, 32, 48, 52, 63, 64, 95] {
            let mut corrupted = *first_cursor;
            corrupted[index] ^= 1;
            assert!(
                owner
                    .list_issues(summary.verification_id(), 1, Some(&corrupted))
                    .is_err()
            );
        }
        let second = owner
            .list_issues(summary.verification_id(), 1, Some(first_cursor))
            .unwrap();
        assert_eq!(second.page().issues()[0].ordinal(), 2);

        for _ in 0..4 {
            let mut run = owner.begin(VerificationMode::Normal).unwrap();
            run.bind_snapshot(8).unwrap();
            run.finish(false).unwrap();
        }
        assert!(matches!(
            owner.list_issues(summary.verification_id(), 1, None),
            Err(AssetStoreError::NotFound)
        ));
    }

    #[test]
    fn cancelled_run_publishes_nothing_and_releases_admission() {
        let owner = owner();
        let id = {
            let run = owner.begin(VerificationMode::Deep).unwrap();
            run.verification_id
        };
        assert!(matches!(
            owner.list_issues(id, 32, None),
            Err(AssetStoreError::NotFound)
        ));
        assert!(owner.begin(VerificationMode::Normal).is_ok());
    }

    #[test]
    fn service_scans_each_bounded_stage_and_publishes_only_after_completion() {
        let owner = owner();
        let store = Arc::new(FakeStore {
            scans: AtomicUsize::new(0),
        });
        let service =
            VerificationService::new(Arc::clone(&store), Arc::new(FakePhysical), owner.clone());
        let summary = block_on_ready(service.verify(VerificationMode::Normal)).unwrap();
        assert_eq!(summary.snapshot_commit_sequence(), 9);
        assert_eq!(summary.discovered_issue_count(), 1);
        assert_eq!(store.scans.load(Ordering::Relaxed), 2);
        let listed = owner
            .list_issues(summary.verification_id(), 32, None)
            .unwrap();
        assert_eq!(listed.page().issues().len(), 1);
    }

    #[test]
    fn service_rejects_pre_dispatch_cancellation_without_report_or_scan() {
        let owner = owner();
        let store = Arc::new(FakeStore {
            scans: AtomicUsize::new(0),
        });
        let service =
            VerificationService::new(Arc::clone(&store), Arc::new(FakePhysical), owner.clone());
        assert_eq!(
            block_on_ready(
                service.verify_controlled(VerificationMode::Deep, Arc::new(AlreadyCancelled),)
            ),
            Err(AssetStoreError::OperationCancelled)
        );
        assert_eq!(store.scans.load(Ordering::Relaxed), 0);
        assert!(owner.begin(VerificationMode::Normal).is_ok());
    }
}
