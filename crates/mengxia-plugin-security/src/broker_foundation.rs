//! Pure comparison of caller-supplied Broker facts, never authenticated authority.
//! No candidate can read bytes, issue/consume a lease, or start a plugin.
use crate::{
    ReviewAssessment, ReviewDenial, ReviewEligibilityCandidate, ReviewFact, ReviewIdentity,
    ReviewIdentityField, ReviewValidity, evaluate_review_eligibility,
};
use mengxia_plugin_package::InspectedPluginPackage;
use mengxia_types::{Id, Sha256Digest};
use std::fmt;

pub const READ_BYTES_PER_REQUEST_MAX: u64 = 1_048_576;
pub const BROKER_VALUE_BYTES_MAX: usize = 16_384;

mod sealed {
    pub trait Marker {}
}
pub trait BrokerMarker: sealed::Marker {}
macro_rules! markers {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {}
        impl sealed::Marker for $name {}
        impl BrokerMarker for $name {}
    )+};
}
markers!(
    Library,
    Project,
    Run,
    Instance,
    Channel,
    LeaseRecord,
    Grant,
    AssetRevision,
    Representation,
    Resource,
    Request,
    Correlation
);

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerKey<K: BrokerMarker>(Id<K>);
impl<K: BrokerMarker> BrokerKey<K> {
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, BrokerValueError> {
        Id::from_bytes(bytes)
            .map(Self)
            .map_err(|_| BrokerValueError::InvalidKey)
    }
    pub fn to_bytes(&self) -> [u8; 16] {
        self.0.to_bytes()
    }
}
impl<K: BrokerMarker> fmt::Debug for BrokerKey<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrokerKey(REDACTED)")
    }
}
impl<K: BrokerMarker> fmt::Display for BrokerKey<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerDigest(Sha256Digest);
impl BrokerDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Sha256Digest::from_bytes(bytes))
    }
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0.to_bytes()
    }
    pub const fn sha256(self) -> Sha256Digest {
        self.0
    }
}
/// Exact existing ResourceMember ordinal. Zero is valid; no independent Member ID.
/// ```compile_fail
/// use mengxia_plugin_security::broker_foundation::BrokerMemberOrdinal;
/// let invalid = BrokerMemberOrdinal(4096);
/// ```
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerMemberOrdinal(u32);
impl BrokerMemberOrdinal {
    pub const fn new(value: u32) -> Result<Self, BrokerValueError> {
        if value <= 4095 {
            Ok(Self(value))
        } else {
            Err(BrokerValueError::InvalidMemberOrdinal)
        }
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerValueError {
    InvalidKey,
    InvalidMemberOrdinal,
}
macro_rules! redacted {
    ($($name:ident),+ $(,)?) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "(REDACTED)"))
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Debug::fmt(self, f) }
        }
    )+};
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerBinding {
    pub library_key: BrokerKey<Library>,
    pub owner_uid: u32,
    pub project_key: BrokerKey<Project>,
    pub run_key: BrokerKey<Run>,
    pub instance_key: BrokerKey<Instance>,
    pub channel_key: BrokerKey<Channel>,
    pub execution_identity: ReviewIdentity,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerMember {
    pub asset_revision_key: BrokerKey<AssetRevision>,
    pub representation_key: BrokerKey<Representation>,
    pub resource_key: BrokerKey<Resource>,
    pub member_ordinal: BrokerMemberOrdinal,
    pub blob_digest: BrokerDigest,
    pub blob_length: u64,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReadTarget {
    pub member: BrokerMember,
    pub offset: u64,
    pub length: u64,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReadAuthorizationKey {
    pub binding: BrokerBinding,
    pub member: BrokerMember,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerReadRequest {
    pub request_key: BrokerKey<Request>,
    pub correlation_key: BrokerKey<Correlation>,
    pub lease_record_key: BrokerKey<LeaseRecord>,
    pub target: ReadTarget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunState {
    Active,
    Inactive,
    Cancelled,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Membership {
    Present,
    Absent,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyDisposition {
    Permit,
    Deny,
    NeedsApproval,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerPluginTrustDecision {
    AllowReviewed,
    Deny,
    NeedsApproval,
    UnsupportedProfile,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeDisposition {
    QualifiedReviewed,
    Denied,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseState {
    Active,
    Revoked,
    Consumed,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevocationState {
    Clear,
    Revoked,
    Unknown,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ExecutionFact {
    pub binding: BrokerBinding,
    pub run_state: RunState,
    pub run_revision: u64,
    pub validity: ReviewValidity,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RunInputFact {
    pub authorization_key: ReadAuthorizationKey,
    pub run_revision: u64,
    pub membership: Membership,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReadPolicyFact {
    pub authorization_key: ReadAuthorizationKey,
    pub policy_revision: u64,
    pub grant_key: BrokerKey<Grant>,
    pub grant_revision: u64,
    pub validity: ReviewValidity,
    pub plugin_trust: ReviewFact<BrokerPluginTrustDecision>,
    pub package_policy: PolicyDisposition,
    pub installed_grant: PolicyDisposition,
    pub project_policy: PolicyDisposition,
    pub owner_policy: PolicyDisposition,
    pub data_policy: PolicyDisposition,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReadRuntimeFact {
    pub binding: BrokerBinding,
    pub qualification_digest: BrokerDigest,
    pub validity: ReviewValidity,
    pub disposition: RuntimeDisposition,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReadLeaseFact {
    pub lease_record_key: BrokerKey<LeaseRecord>,
    pub authorization_key: ReadAuthorizationKey,
    pub grant_key: BrokerKey<Grant>,
    pub grant_revision: u64,
    pub policy_revision: u64,
    pub revocation_epoch: u64,
    pub validity: ReviewValidity,
    pub state: LeaseState,
    pub allowed_offset: u64,
    pub allowed_length: u64,
    pub remaining_operations: u64,
    pub remaining_bytes: u64,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReadRevocationFact {
    pub authorization_key: ReadAuthorizationKey,
    pub grant_key: BrokerKey<Grant>,
    pub grant_revision: u64,
    pub policy_revision: u64,
    pub epoch: u64,
    pub validity: ReviewValidity,
    pub state: RevocationState,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerClock {
    pub now_ms: u64,
    pub last_observed_ms: u64,
    pub minimum_revocation_epoch: u64,
}
#[derive(Clone, Copy)]
pub struct BrokerReadAssessment<'a> {
    pub request: BrokerReadRequest,
    pub caller: ReviewFact<BrokerBinding>,
    pub execution: ReviewFact<ExecutionFact>,
    pub input: ReviewFact<RunInputFact>,
    pub package: &'a InspectedPluginPackage,
    pub review: ReviewAssessment,
    pub policy: ReviewFact<ReadPolicyFact>,
    pub runtime: ReviewFact<ReadRuntimeFact>,
    pub lease: ReviewFact<ReadLeaseFact>,
    pub revocation: ReviewFact<ReadRevocationFact>,
    pub clock: ReviewFact<BrokerClock>,
}
impl fmt::Debug for BrokerReadAssessment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrokerReadAssessment(REDACTED)")
    }
}
impl fmt::Display for BrokerReadAssessment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerFactKind {
    Caller,
    Execution,
    Input,
    Policy,
    PluginTrust,
    Runtime,
    Lease,
    Revocation,
    Clock,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnavailableFact {
    Missing,
    Unverified,
    Invalid,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerRevisionField {
    ExecutionRun,
    InputRun,
    PolicyPolicy,
    PolicyGrant,
    LeaseGrant,
    LeasePolicy,
    RevocationGrant,
    RevocationPolicy,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerBindingField {
    Library,
    Owner,
    Project,
    Run,
    Instance,
    Channel,
    Identity(ReviewIdentityField),
    AssetRevision,
    Representation,
    Resource,
    MemberOrdinal,
    BlobDigest,
    BlobLength,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerPolicyFactor {
    Package,
    InstalledGrant,
    Project,
    Owner,
    Data,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerReadDenial {
    FactUnavailable(BrokerFactKind, UnavailableFact),
    InvalidRevision(BrokerRevisionField),
    InvalidValidity(BrokerFactKind),
    InvalidLeaseRange,
    InvalidRequestRange,
    ReadLimitExceeded,
    BindingMismatch(BrokerFactKind, BrokerBindingField),
    RunRevisionMismatch,
    LeaseRecordMismatch,
    GrantKeyMismatch(BrokerFactKind),
    GrantRevisionMismatch(BrokerFactKind),
    PolicyRevisionMismatch(BrokerFactKind),
    ClockRollback,
    NotYetValid(BrokerFactKind),
    Expired(BrokerFactKind),
    RevocationRollback,
    LeaseEpochMismatch,
    RunNotActive,
    NotRunInput,
    LeaseNotActive,
    RevocationDenied,
    ManifestMismatch,
    ReadNotRequested,
    ReviewClockMismatch,
    ReviewRejected(ReviewDenial),
    ReviewIdentityMismatch,
    RuntimeNotQualified,
    PluginTrustDenied(BrokerPluginTrustDecision),
    PolicyDenied(BrokerPolicyFactor, PolicyDisposition),
    LeaseRangeDenied,
    OperationBudgetExhausted,
    ByteBudgetExhausted,
}
/// This private value is comparison evidence, not an authorization token.
/// ```compile_fail
/// use mengxia_plugin_security::broker_foundation::BrokerReadCandidate;
/// let forged = BrokerReadCandidate {};
/// ```
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerReadCandidate {
    request: BrokerReadRequest,
    authorization_key: ReadAuthorizationKey,
    run_revision: u64,
    policy_revision: u64,
    grant_revision: u64,
    grant_key: BrokerKey<Grant>,
    revocation_epoch: u64,
    runtime_qualification_digest: BrokerDigest,
    review: ReviewEligibilityCandidate,
    plugin_trust_decision: BrokerPluginTrustDecision,
    evaluated_at: u64,
    valid_until: u64,
}
macro_rules! getters {
    ($($field:ident: $ty:ty),+ $(,)?) => {$(
        pub const fn $field(&self) -> $ty { self.$field }
    )+};
}
impl BrokerReadCandidate {
    getters!(request: BrokerReadRequest, authorization_key: ReadAuthorizationKey,
        run_revision: u64, policy_revision: u64, grant_revision: u64, grant_key: BrokerKey<Grant>,
        revocation_epoch: u64, runtime_qualification_digest: BrokerDigest,
        review: ReviewEligibilityCandidate, plugin_trust_decision: BrokerPluginTrustDecision,
        evaluated_at: u64, valid_until: u64);
}
#[derive(Clone, Copy, Eq, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "The accepted bounded candidate stays inline to keep evaluation allocation-free; size is regression-tested"
)]
pub enum BrokerReadDecision {
    Eligible(BrokerReadCandidate),
    Denied(BrokerReadDenial),
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerReadEvaluation {
    pub decision: BrokerReadDecision,
    pub audit: BrokerReadAuditCandidate,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerAuditDecision {
    Eligible,
    Denied(BrokerReadDenial),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerAuditAction {
    ReadRunInput,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditAssignment {
    NotAssigned,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditDelegation {
    NotEstablished,
}
/// None means unavailable/unknown. Some is only CallerAsserted, never authenticated.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CallerAsserted<T>(T);
impl<T: Copy> CallerAsserted<T> {
    pub const fn value(&self) -> T {
        self.0
    }
}
impl<T> fmt::Debug for CallerAsserted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CallerAsserted(REDACTED)")
    }
}
impl<T> fmt::Display for CallerAsserted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerReadAuditCandidate {
    requested: BrokerReadRequest,
    decision: BrokerAuditDecision,
    caller: Option<CallerAsserted<BrokerBinding>>,
    evaluated_at: Option<CallerAsserted<u64>>,
    run_revision: Option<CallerAsserted<u64>>,
    policy_revision: Option<CallerAsserted<u64>>,
    grant_revision: Option<CallerAsserted<u64>>,
    runtime_qualification_digest: Option<CallerAsserted<BrokerDigest>>,
    review_approval: Option<CallerAsserted<BrokerDigest>>,
    review_evidence: Option<CallerAsserted<BrokerDigest>>,
    review_policy: Option<CallerAsserted<u64>>,
    review_snapshot_epoch: Option<CallerAsserted<u64>>,
    plugin_trust: Option<ReviewFact<BrokerPluginTrustDecision>>,
}
impl BrokerReadAuditCandidate {
    getters!(requested: BrokerReadRequest, decision: BrokerAuditDecision,
        caller: Option<CallerAsserted<BrokerBinding>>, evaluated_at: Option<CallerAsserted<u64>>,
        run_revision: Option<CallerAsserted<u64>>, policy_revision: Option<CallerAsserted<u64>>,
        grant_revision: Option<CallerAsserted<u64>>, runtime_qualification_digest: Option<CallerAsserted<BrokerDigest>>,
        review_approval: Option<CallerAsserted<BrokerDigest>>, review_evidence: Option<CallerAsserted<BrokerDigest>>,
        review_policy: Option<CallerAsserted<u64>>, review_snapshot_epoch: Option<CallerAsserted<u64>>,
        plugin_trust: Option<ReviewFact<BrokerPluginTrustDecision>>);
    pub const fn schema_version(&self) -> u32 {
        1
    }
    pub const fn action(&self) -> BrokerAuditAction {
        BrokerAuditAction::ReadRunInput
    }
    pub const fn delegation(&self) -> AuditDelegation {
        AuditDelegation::NotEstablished
    }
    pub const fn event_id(&self) -> AuditAssignment {
        AuditAssignment::NotAssigned
    }
    pub const fn recorded_at(&self) -> AuditAssignment {
        AuditAssignment::NotAssigned
    }
    pub const fn persistence(&self) -> AuditAssignment {
        AuditAssignment::NotAssigned
    }
}
redacted!(
    BrokerDigest,
    BrokerMemberOrdinal,
    BrokerBinding,
    BrokerMember,
    ReadTarget,
    ReadAuthorizationKey,
    BrokerReadRequest,
    ExecutionFact,
    RunInputFact,
    ReadPolicyFact,
    ReadRuntimeFact,
    ReadLeaseFact,
    ReadRevocationFact,
    BrokerClock,
    BrokerReadCandidate,
    BrokerReadDecision,
    BrokerReadEvaluation,
    BrokerReadAuditCandidate
);

fn checked<T: Copy>(fact: ReviewFact<T>, kind: BrokerFactKind) -> Result<T, BrokerReadDenial> {
    match fact {
        ReviewFact::Checked(v) => Ok(v),
        ReviewFact::Missing => Err(BrokerReadDenial::FactUnavailable(
            kind,
            UnavailableFact::Missing,
        )),
        ReviewFact::Unverified => Err(BrokerReadDenial::FactUnavailable(
            kind,
            UnavailableFact::Unverified,
        )),
        ReviewFact::Invalid => Err(BrokerReadDenial::FactUnavailable(
            kind,
            UnavailableFact::Invalid,
        )),
    }
}
fn supplied<T>(fact: ReviewFact<T>) -> Option<T> {
    if let ReviewFact::Checked(v) = fact {
        Some(v)
    } else {
        None
    }
}
fn valid_interval(v: ReviewValidity) -> bool {
    v.start_ms < v.end_ms
}
fn valid_range(offset: u64, length: u64, total: u64) -> bool {
    length > 0 && offset <= total && length <= total - offset
}
fn binding_matches(
    actual: BrokerBinding,
    expected: BrokerBinding,
    source: BrokerFactKind,
) -> Result<(), BrokerReadDenial> {
    use BrokerBindingField as F;
    let a = actual.execution_identity;
    let e = expected.execution_identity;
    for (field, equal) in [
        (F::Library, actual.library_key == expected.library_key),
        (F::Owner, actual.owner_uid == expected.owner_uid),
        (F::Project, actual.project_key == expected.project_key),
        (F::Run, actual.run_key == expected.run_key),
        (F::Instance, actual.instance_key == expected.instance_key),
        (F::Channel, actual.channel_key == expected.channel_key),
        (
            F::Identity(ReviewIdentityField::Manifest),
            a.manifest_digest == e.manifest_digest,
        ),
        (
            F::Identity(ReviewIdentityField::Artifact),
            a.artifact_digest == e.artifact_digest,
        ),
        (
            F::Identity(ReviewIdentityField::DependencyClosure),
            a.dependency_closure_digest == e.dependency_closure_digest,
        ),
        (
            F::Identity(ReviewIdentityField::Permissions),
            a.permissions_digest == e.permissions_digest,
        ),
        (
            F::Identity(ReviewIdentityField::Profile),
            a.profile_digest == e.profile_digest,
        ),
        (
            F::Identity(ReviewIdentityField::PlatformTuple),
            a.platform_tuple_digest == e.platform_tuple_digest,
        ),
    ] {
        if !equal {
            return Err(BrokerReadDenial::BindingMismatch(source, field));
        }
    }
    Ok(())
}
fn key_matches(
    actual: ReadAuthorizationKey,
    expected: ReadAuthorizationKey,
    source: BrokerFactKind,
) -> Result<(), BrokerReadDenial> {
    binding_matches(actual.binding, expected.binding, source)?;
    use BrokerBindingField as F;
    let a = actual.member;
    let e = expected.member;
    for (field, equal) in [
        (
            F::AssetRevision,
            a.asset_revision_key == e.asset_revision_key,
        ),
        (
            F::Representation,
            a.representation_key == e.representation_key,
        ),
        (F::Resource, a.resource_key == e.resource_key),
        (F::MemberOrdinal, a.member_ordinal == e.member_ordinal),
        (F::BlobDigest, a.blob_digest == e.blob_digest),
        (F::BlobLength, a.blob_length == e.blob_length),
    ] {
        if !equal {
            return Err(BrokerReadDenial::BindingMismatch(source, field));
        }
    }
    Ok(())
}
fn assess(a: &BrokerReadAssessment<'_>) -> Result<BrokerReadCandidate, BrokerReadDenial> {
    use BrokerFactKind as K;
    use BrokerReadDenial as D;
    let caller = checked(a.caller, K::Caller)?;
    let execution = checked(a.execution, K::Execution)?;
    let input = checked(a.input, K::Input)?;
    let policy = checked(a.policy, K::Policy)?;
    let trust = checked(policy.plugin_trust, K::PluginTrust)?;
    let runtime = checked(a.runtime, K::Runtime)?;
    let lease = checked(a.lease, K::Lease)?;
    let revocation = checked(a.revocation, K::Revocation)?;
    let clock = checked(a.clock, K::Clock)?;
    use BrokerRevisionField as R;
    for (field, value) in [
        (R::ExecutionRun, execution.run_revision),
        (R::InputRun, input.run_revision),
        (R::PolicyPolicy, policy.policy_revision),
        (R::PolicyGrant, policy.grant_revision),
        (R::LeaseGrant, lease.grant_revision),
        (R::LeasePolicy, lease.policy_revision),
        (R::RevocationGrant, revocation.grant_revision),
        (R::RevocationPolicy, revocation.policy_revision),
    ] {
        if value == 0 {
            return Err(D::InvalidRevision(field));
        }
    }
    let intervals = [
        (K::Execution, execution.validity),
        (K::Policy, policy.validity),
        (K::Runtime, runtime.validity),
        (K::Lease, lease.validity),
        (K::Revocation, revocation.validity),
    ];
    for (kind, validity) in intervals {
        if !valid_interval(validity) {
            return Err(D::InvalidValidity(kind));
        }
    }
    if !valid_range(
        lease.allowed_offset,
        lease.allowed_length,
        lease.authorization_key.member.blob_length,
    ) {
        return Err(D::InvalidLeaseRange);
    }
    let request = a.request;
    if request.target.length > READ_BYTES_PER_REQUEST_MAX {
        return Err(D::ReadLimitExceeded);
    }
    if !valid_range(
        request.target.offset,
        request.target.length,
        request.target.member.blob_length,
    ) {
        return Err(D::InvalidRequestRange);
    }
    let key = ReadAuthorizationKey {
        binding: caller,
        member: request.target.member,
    };
    binding_matches(execution.binding, caller, K::Execution)?;
    key_matches(input.authorization_key, key, K::Input)?;
    key_matches(policy.authorization_key, key, K::Policy)?;
    binding_matches(runtime.binding, caller, K::Runtime)?;
    key_matches(lease.authorization_key, key, K::Lease)?;
    key_matches(revocation.authorization_key, key, K::Revocation)?;
    if input.run_revision != execution.run_revision {
        return Err(D::RunRevisionMismatch);
    }
    if request.lease_record_key != lease.lease_record_key {
        return Err(D::LeaseRecordMismatch);
    }
    for (kind, key) in [
        (K::Lease, lease.grant_key),
        (K::Revocation, revocation.grant_key),
    ] {
        if key != policy.grant_key {
            return Err(D::GrantKeyMismatch(kind));
        }
    }
    for (kind, revision) in [
        (K::Lease, lease.grant_revision),
        (K::Revocation, revocation.grant_revision),
    ] {
        if revision != policy.grant_revision {
            return Err(D::GrantRevisionMismatch(kind));
        }
    }
    for (kind, revision) in [
        (K::Lease, lease.policy_revision),
        (K::Revocation, revocation.policy_revision),
    ] {
        if revision != policy.policy_revision {
            return Err(D::PolicyRevisionMismatch(kind));
        }
    }
    if clock.now_ms < clock.last_observed_ms {
        return Err(D::ClockRollback);
    }
    for (kind, validity) in intervals {
        if clock.now_ms < validity.start_ms {
            return Err(D::NotYetValid(kind));
        }
        if clock.now_ms >= validity.end_ms {
            return Err(D::Expired(kind));
        }
    }
    if revocation.epoch < clock.minimum_revocation_epoch {
        return Err(D::RevocationRollback);
    }
    if lease.revocation_epoch != revocation.epoch {
        return Err(D::LeaseEpochMismatch);
    }
    if execution.run_state != RunState::Active {
        return Err(D::RunNotActive);
    }
    if input.membership != Membership::Present {
        return Err(D::NotRunInput);
    }
    if lease.state != LeaseState::Active {
        return Err(D::LeaseNotActive);
    }
    if revocation.state != RevocationState::Clear {
        return Err(D::RevocationDenied);
    }
    if a.package.digest().sha256() != caller.execution_identity.manifest_digest {
        return Err(D::ManifestMismatch);
    }
    let permissions = a.package.requested_permissions();
    if permissions.len() != 1
        || permissions[0].kind() != "broker.asset.read@1"
        || permissions[0].scope() != "run-inputs"
    {
        return Err(D::ReadNotRequested);
    }
    if let ReviewFact::Checked(review_clock) = a.review.clock
        && review_clock.now_ms != clock.now_ms
    {
        return Err(D::ReviewClockMismatch);
    }
    let review = evaluate_review_eligibility(&a.review).map_err(D::ReviewRejected)?;
    if review.identity() != caller.execution_identity {
        return Err(D::ReviewIdentityMismatch);
    }
    if runtime.disposition != RuntimeDisposition::QualifiedReviewed {
        return Err(D::RuntimeNotQualified);
    }
    if trust != BrokerPluginTrustDecision::AllowReviewed {
        return Err(D::PluginTrustDenied(trust));
    }
    use BrokerPolicyFactor as P;
    for (factor, disposition) in [
        (P::Package, policy.package_policy),
        (P::InstalledGrant, policy.installed_grant),
        (P::Project, policy.project_policy),
        (P::Owner, policy.owner_policy),
        (P::Data, policy.data_policy),
    ] {
        if disposition != PolicyDisposition::Permit {
            return Err(D::PolicyDenied(factor, disposition));
        }
    }
    if request.target.offset < lease.allowed_offset {
        return Err(D::LeaseRangeDenied);
    }
    let delta = request.target.offset - lease.allowed_offset;
    if delta > lease.allowed_length || request.target.length > lease.allowed_length - delta {
        return Err(D::LeaseRangeDenied);
    }
    if lease.remaining_operations == 0 {
        return Err(D::OperationBudgetExhausted);
    }
    if lease.remaining_bytes < request.target.length {
        return Err(D::ByteBudgetExhausted);
    }
    let mut valid_until = review.valid_until_ms();
    for (_, validity) in intervals {
        valid_until = valid_until.min(validity.end_ms);
    }
    Ok(BrokerReadCandidate {
        request,
        authorization_key: key,
        run_revision: execution.run_revision,
        policy_revision: policy.policy_revision,
        grant_revision: policy.grant_revision,
        grant_key: policy.grant_key,
        revocation_epoch: revocation.epoch,
        runtime_qualification_digest: runtime.qualification_digest,
        review,
        plugin_trust_decision: trust,
        evaluated_at: clock.now_ms,
        valid_until,
    })
}
/// Synchronous, allocation-free, deterministic evaluation. Repeating it consumes nothing.
pub fn evaluate_broker_read(a: &BrokerReadAssessment<'_>) -> BrokerReadEvaluation {
    let result = assess(a);
    let policy = supplied(a.policy)
        .filter(|p| p.policy_revision > 0 && p.grant_revision > 0 && valid_interval(p.validity));
    let execution =
        supplied(a.execution).filter(|e| e.run_revision > 0 && valid_interval(e.validity));
    let runtime = supplied(a.runtime).filter(|r| valid_interval(r.validity));
    let approval = supplied(a.review.approval).filter(|r| valid_interval(r.validity));
    let snapshot = supplied(a.review.revocation).filter(|r| valid_interval(r.validity));
    let audit = BrokerReadAuditCandidate {
        requested: a.request,
        decision: match result {
            Ok(_) => BrokerAuditDecision::Eligible,
            Err(e) => BrokerAuditDecision::Denied(e),
        },
        caller: supplied(a.caller).map(CallerAsserted),
        evaluated_at: supplied(a.clock)
            .filter(|c| c.now_ms >= c.last_observed_ms)
            .map(|c| CallerAsserted(c.now_ms)),
        run_revision: execution.map(|e| CallerAsserted(e.run_revision)),
        policy_revision: policy.map(|p| CallerAsserted(p.policy_revision)),
        grant_revision: policy.map(|p| CallerAsserted(p.grant_revision)),
        runtime_qualification_digest: runtime.map(|r| CallerAsserted(r.qualification_digest)),
        review_approval: approval.map(|r| CallerAsserted(BrokerDigest(r.record_digest))),
        review_evidence: approval.map(|r| CallerAsserted(BrokerDigest(r.evidence_digest))),
        review_policy: approval.map(|r| CallerAsserted(r.policy_version)),
        review_snapshot_epoch: snapshot.map(|r| CallerAsserted(r.epoch)),
        plugin_trust: policy.map(|p| p.plugin_trust),
    };
    BrokerReadEvaluation {
        decision: match result {
            Ok(v) => BrokerReadDecision::Eligible(v),
            Err(e) => BrokerReadDecision::Denied(e),
        },
        audit,
    }
}
