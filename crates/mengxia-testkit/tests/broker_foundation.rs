use mengxia_plugin_package::{InspectedPluginPackage, inspect_manifest};
use mengxia_plugin_security::broker_foundation::*;
use mengxia_plugin_security::{
    ReviewApproval, ReviewAssessment, ReviewClock, ReviewDenial, ReviewFact, ReviewFactKind,
    ReviewIdentity, ReviewIdentityField, ReviewRequirement, ReviewRevocationSnapshot,
    ReviewRevocationStatus, ReviewValidity,
};

fn key<K: BrokerMarker>(tag: u8) -> BrokerKey<K> {
    let mut bytes = [0; 16];
    bytes[6] = 0x70;
    bytes[8] = 0x80;
    bytes[15] = tag;
    BrokerKey::from_bytes(bytes).unwrap()
}
fn digest(tag: u8) -> BrokerDigest {
    BrokerDigest::from_bytes([tag; 32])
}
fn package() -> InspectedPluginPackage {
    inspect_manifest(include_bytes!("fixtures/task_010/manifest-v1.golden.json")).unwrap()
}
fn checked<T>(v: &mut ReviewFact<T>) -> &mut T {
    match v {
        ReviewFact::Checked(v) => v,
        _ => panic!("fixture must be checked"),
    }
}
fn fixture(package: &InspectedPluginPackage) -> BrokerReadAssessment<'_> {
    let identity = ReviewIdentity {
        manifest_digest: package.digest().sha256(),
        artifact_digest: digest(2).sha256(),
        dependency_closure_digest: digest(3).sha256(),
        permissions_digest: digest(4).sha256(),
        profile_digest: digest(5).sha256(),
        platform_tuple_digest: digest(6).sha256(),
    };
    let binding = BrokerBinding {
        library_key: key(1),
        owner_uid: 501,
        project_key: key(2),
        run_key: key(3),
        instance_key: key(4),
        channel_key: key(5),
        execution_identity: identity,
    };
    let member = BrokerMember {
        asset_revision_key: key(6),
        representation_key: key(7),
        resource_key: key(8),
        member_ordinal: BrokerMemberOrdinal::new(0).unwrap(),
        blob_digest: digest(7),
        blob_length: 2 * READ_BYTES_PER_REQUEST_MAX,
    };
    let authorization_key = ReadAuthorizationKey { binding, member };
    let validity = ReviewValidity {
        start_ms: 100,
        end_ms: 400,
    };
    let permit = PolicyDisposition::Permit;
    BrokerReadAssessment {
        request: BrokerReadRequest {
            request_key: key(9),
            correlation_key: key(10),
            lease_record_key: key(11),
            target: ReadTarget {
                member,
                offset: 8,
                length: 16,
            },
        },
        caller: ReviewFact::Checked(binding),
        execution: ReviewFact::Checked(ExecutionFact {
            binding,
            run_state: RunState::Active,
            run_revision: 1,
            validity,
        }),
        input: ReviewFact::Checked(RunInputFact {
            authorization_key,
            run_revision: 1,
            membership: Membership::Present,
        }),
        package,
        review: ReviewAssessment {
            requirement: ReviewFact::Checked(ReviewRequirement {
                identity,
                policy_version: 1,
                evidence_digest: digest(8).sha256(),
            }),
            approval: ReviewFact::Checked(ReviewApproval {
                identity,
                policy_version: 1,
                evidence_digest: digest(8).sha256(),
                record_digest: digest(9).sha256(),
                validity,
            }),
            revocation: ReviewFact::Checked(ReviewRevocationSnapshot {
                approval_record_digest: digest(9).sha256(),
                status: ReviewRevocationStatus::Clear,
                epoch: 5,
                validity: ReviewValidity {
                    start_ms: 100,
                    end_ms: 300,
                },
            }),
            clock: ReviewFact::Checked(ReviewClock {
                now_ms: 150,
                last_observed_ms: 140,
                minimum_snapshot_epoch: 5,
            }),
        },
        policy: ReviewFact::Checked(ReadPolicyFact {
            authorization_key,
            policy_revision: 1,
            grant_key: key(12),
            grant_revision: 1,
            validity,
            plugin_trust: ReviewFact::Checked(BrokerPluginTrustDecision::AllowReviewed),
            package_policy: permit,
            installed_grant: permit,
            project_policy: permit,
            owner_policy: permit,
            data_policy: permit,
        }),
        runtime: ReviewFact::Checked(ReadRuntimeFact {
            binding,
            qualification_digest: digest(10),
            validity,
            disposition: RuntimeDisposition::QualifiedReviewed,
        }),
        lease: ReviewFact::Checked(ReadLeaseFact {
            lease_record_key: key(11),
            authorization_key,
            grant_key: key(12),
            grant_revision: 1,
            policy_revision: 1,
            revocation_epoch: 5,
            validity,
            state: LeaseState::Active,
            allowed_offset: 0,
            allowed_length: member.blob_length,
            remaining_operations: 2,
            remaining_bytes: member.blob_length,
        }),
        revocation: ReviewFact::Checked(ReadRevocationFact {
            authorization_key,
            grant_key: key(12),
            grant_revision: 1,
            policy_revision: 1,
            epoch: 5,
            validity,
            state: RevocationState::Clear,
        }),
        clock: ReviewFact::Checked(BrokerClock {
            now_ms: 150,
            last_observed_ms: 140,
            minimum_revocation_epoch: 5,
        }),
    }
}
fn denied(a: &BrokerReadAssessment<'_>, reason: BrokerReadDenial) {
    let result = evaluate_broker_read(a);
    assert_eq!(result.decision, BrokerReadDecision::Denied(reason));
    assert_eq!(result.audit.decision(), BrokerAuditDecision::Denied(reason));
    assert_eq!(result.audit.requested(), a.request);
    assert_eq!(result.audit.event_id(), AuditAssignment::NotAssigned);
    assert_eq!(result.audit.recorded_at(), AuditAssignment::NotAssigned);
    assert_eq!(result.audit.persistence(), AuditAssignment::NotAssigned);
    assert_eq!(result.audit.delegation(), AuditDelegation::NotEstablished);
    assert_eq!(format!("{result:?}"), "BrokerReadEvaluation(REDACTED)");
    assert_eq!(evaluate_broker_read(a), result);
}
fn eligible(a: &BrokerReadAssessment<'_>) -> BrokerReadCandidate {
    let result = evaluate_broker_read(a);
    assert_eq!(result.audit.decision(), BrokerAuditDecision::Eligible);
    match result.decision {
        BrokerReadDecision::Eligible(c) => c,
        BrokerReadDecision::Denied(d) => panic!("unexpected {d:?}"),
    }
}
fn absent<T>(state: usize) -> ReviewFact<T> {
    match state {
        0 => ReviewFact::Missing,
        1 => ReviewFact::Unverified,
        _ => ReviewFact::Invalid,
    }
}

#[test]
fn valid_boundaries_produce_only_immutable_redacted_candidates() {
    let p = package();
    let mut a = fixture(&p);
    let c = eligible(&a);
    assert_eq!(c.request(), a.request);
    assert_eq!(
        c.authorization_key(),
        checked(&mut a.input).authorization_key
    );
    assert_eq!(c.run_revision(), 1);
    assert_eq!(c.policy_revision(), 1);
    assert_eq!(c.grant_revision(), 1);
    assert_eq!(c.grant_key(), key(12));
    assert_eq!(c.revocation_epoch(), 5);
    assert_eq!(c.runtime_qualification_digest(), digest(10));
    assert_eq!(
        c.plugin_trust_decision(),
        BrokerPluginTrustDecision::AllowReviewed
    );
    assert_eq!(
        c.review().identity(),
        checked(&mut a.caller).execution_identity
    );
    assert_eq!(c.evaluated_at(), 150);
    assert_eq!(c.valid_until(), 300);
    let audit = evaluate_broker_read(&a).audit;
    assert_eq!(audit.schema_version(), 1);
    assert_eq!(audit.action(), BrokerAuditAction::ReadRunInput);
    assert_eq!(audit.caller().unwrap().value(), *checked(&mut a.caller));
    assert_eq!(audit.evaluated_at().unwrap().value(), 150);
    assert_eq!(audit.run_revision().unwrap().value(), 1);
    assert_eq!(audit.policy_revision().unwrap().value(), 1);
    assert_eq!(audit.grant_revision().unwrap().value(), 1);
    assert_eq!(
        audit.runtime_qualification_digest().unwrap().value(),
        digest(10)
    );
    assert_eq!(audit.review_approval().unwrap().value(), digest(9));
    assert_eq!(audit.review_evidence().unwrap().value(), digest(8));
    assert_eq!(audit.review_policy().unwrap().value(), 1);
    assert_eq!(audit.review_snapshot_epoch().unwrap().value(), 5);
    assert_eq!(
        audit.plugin_trust(),
        Some(ReviewFact::Checked(
            BrokerPluginTrustDecision::AllowReviewed
        ))
    );
    for ordinal in [0, 4095] {
        let mut a = fixture(&p);
        set_member(&mut a, |m| {
            m.member_ordinal = BrokerMemberOrdinal::new(ordinal).unwrap()
        });
        assert_eq!(
            eligible(&a).authorization_key().member.member_ordinal.get(),
            ordinal
        );
    }
    for (offset, length) in [
        (0, 1),
        (0, READ_BYTES_PER_REQUEST_MAX),
        (READ_BYTES_PER_REQUEST_MAX, READ_BYTES_PER_REQUEST_MAX),
    ] {
        a.request.target.offset = offset;
        a.request.target.length = length;
        assert_eq!(eligible(&a).request().target.length, length);
    }
    assert!(std::mem::size_of::<BrokerReadAssessment<'_>>() <= BROKER_VALUE_BYTES_MAX);
    assert!(std::mem::size_of::<BrokerReadEvaluation>() <= BROKER_VALUE_BYTES_MAX);
}
#[test]
fn constructors_preserve_existing_uuid_and_member_ordinal_contracts() {
    assert_eq!(
        BrokerKey::<Run>::from_bytes([0; 16]),
        Err(BrokerValueError::InvalidKey)
    );
    let bytes = key::<Run>(99).to_bytes();
    assert_eq!(
        BrokerKey::<Run>::from_bytes(bytes).unwrap().to_bytes(),
        bytes
    );
    for (index, wrong) in [(6, 0x40), (8, 0)] {
        let mut bad = bytes;
        bad[index] = wrong;
        assert_eq!(
            BrokerKey::<Run>::from_bytes(bad),
            Err(BrokerValueError::InvalidKey)
        );
    }
    for value in [4096, u32::MAX] {
        assert_eq!(
            BrokerMemberOrdinal::new(value),
            Err(BrokerValueError::InvalidMemberOrdinal)
        );
    }
    assert_eq!(digest(0).to_bytes(), [0; 32]);
    assert_eq!(format!("{}", key::<Run>(99)), "BrokerKey(REDACTED)");
    assert_eq!(
        format!("{}", BrokerMemberOrdinal::new(0).unwrap()),
        "BrokerMemberOrdinal(REDACTED)"
    );
}
#[test]
fn all_unavailable_facts_deny_without_panic_or_inherited_trust() {
    use BrokerFactKind as K;
    for kind in [
        K::Caller,
        K::Execution,
        K::Input,
        K::Policy,
        K::PluginTrust,
        K::Runtime,
        K::Lease,
        K::Revocation,
        K::Clock,
    ] {
        for (state, expected) in [
            UnavailableFact::Missing,
            UnavailableFact::Unverified,
            UnavailableFact::Invalid,
        ]
        .into_iter()
        .enumerate()
        {
            let p = package();
            let mut a = fixture(&p);
            match kind {
                K::Caller => a.caller = absent(state),
                K::Execution => a.execution = absent(state),
                K::Input => a.input = absent(state),
                K::Policy => a.policy = absent(state),
                K::PluginTrust => checked(&mut a.policy).plugin_trust = absent(state),
                K::Runtime => a.runtime = absent(state),
                K::Lease => a.lease = absent(state),
                K::Revocation => a.revocation = absent(state),
                K::Clock => a.clock = absent(state),
            }
            denied(&a, BrokerReadDenial::FactUnavailable(kind, expected));
        }
    }
}
fn bindings<'a>(a: &'a mut BrokerReadAssessment<'_>, source: usize) -> &'a mut BrokerBinding {
    match source {
        0 => checked(&mut a.caller),
        1 => &mut checked(&mut a.execution).binding,
        2 => &mut checked(&mut a.input).authorization_key.binding,
        3 => &mut checked(&mut a.policy).authorization_key.binding,
        4 => &mut checked(&mut a.runtime).binding,
        5 => &mut checked(&mut a.lease).authorization_key.binding,
        _ => &mut checked(&mut a.revocation).authorization_key.binding,
    }
}
fn change_identity(i: &mut ReviewIdentity, field: usize) {
    let d = digest(255).sha256();
    match field {
        0 => i.manifest_digest = d,
        1 => i.artifact_digest = d,
        2 => i.dependency_closure_digest = d,
        3 => i.permissions_digest = d,
        4 => i.profile_digest = d,
        _ => i.platform_tuple_digest = d,
    }
}
#[test]
fn every_binding_and_executable_identity_field_is_independently_compared() {
    use BrokerBindingField as F;
    use BrokerFactKind as K;
    let fields = [
        F::Library,
        F::Owner,
        F::Project,
        F::Run,
        F::Instance,
        F::Channel,
        F::Identity(ReviewIdentityField::Manifest),
        F::Identity(ReviewIdentityField::Artifact),
        F::Identity(ReviewIdentityField::DependencyClosure),
        F::Identity(ReviewIdentityField::Permissions),
        F::Identity(ReviewIdentityField::Profile),
        F::Identity(ReviewIdentityField::PlatformTuple),
    ];
    let kinds = [
        K::Execution,
        K::Execution,
        K::Input,
        K::Policy,
        K::Runtime,
        K::Lease,
        K::Revocation,
    ];
    let p = package();
    for (source, kind) in kinds.into_iter().enumerate() {
        for (field, expected) in fields.into_iter().enumerate() {
            let mut a = fixture(&p);
            let b = bindings(&mut a, source);
            match field {
                0 => b.library_key = key(99),
                1 => b.owner_uid = 777,
                2 => b.project_key = key(99),
                3 => b.run_key = key(99),
                4 => b.instance_key = key(99),
                5 => b.channel_key = key(99),
                _ => change_identity(&mut b.execution_identity, field - 6),
            }
            denied(&a, BrokerReadDenial::BindingMismatch(kind, expected));
        }
    }
}
fn member<'a>(a: &'a mut BrokerReadAssessment<'_>, source: usize) -> &'a mut BrokerMember {
    match source {
        0 => &mut a.request.target.member,
        1 => &mut checked(&mut a.input).authorization_key.member,
        2 => &mut checked(&mut a.policy).authorization_key.member,
        3 => &mut checked(&mut a.lease).authorization_key.member,
        _ => &mut checked(&mut a.revocation).authorization_key.member,
    }
}
fn set_member(a: &mut BrokerReadAssessment<'_>, mut f: impl FnMut(&mut BrokerMember)) {
    for source in 0..5 {
        f(member(a, source));
    }
}
#[test]
fn resource_ordinal_and_asset_chain_do_not_collapse_equal_blobs() {
    use BrokerBindingField as F;
    use BrokerFactKind as K;
    let p = package();
    let fields = [
        F::AssetRevision,
        F::Representation,
        F::Resource,
        F::MemberOrdinal,
        F::BlobDigest,
        F::BlobLength,
    ];
    for (source, kind) in [K::Input, K::Input, K::Policy, K::Lease, K::Revocation]
        .into_iter()
        .enumerate()
    {
        for (index, field) in fields.into_iter().enumerate() {
            let mut a = fixture(&p);
            let m = member(&mut a, source);
            match index {
                0 => m.asset_revision_key = key(99),
                1 => m.representation_key = key(99),
                2 => m.resource_key = key(99),
                3 => m.member_ordinal = BrokerMemberOrdinal::new(1).unwrap(),
                4 => m.blob_digest = digest(99),
                _ => m.blob_length += 1,
            }
            denied(&a, BrokerReadDenial::BindingMismatch(kind, field));
        }
    }
}
#[test]
fn plugin_trust_and_project_trust_have_full_independent_cross_product() {
    use BrokerPluginTrustDecision as T;
    use PolicyDisposition as P;
    let p = package();
    for trust in [
        T::AllowReviewed,
        T::Deny,
        T::NeedsApproval,
        T::UnsupportedProfile,
        T::Unknown,
    ] {
        for project in [P::Permit, P::Deny, P::NeedsApproval, P::Unknown] {
            let mut a = fixture(&p);
            let policy = checked(&mut a.policy);
            policy.plugin_trust = ReviewFact::Checked(trust);
            policy.project_policy = project;
            if trust != T::AllowReviewed {
                denied(&a, BrokerReadDenial::PluginTrustDenied(trust));
            } else if project != P::Permit {
                denied(
                    &a,
                    BrokerReadDenial::PolicyDenied(BrokerPolicyFactor::Project, project),
                );
            } else {
                eligible(&a);
            }
        }
    }
    for (state, reason) in [
        UnavailableFact::Missing,
        UnavailableFact::Unverified,
        UnavailableFact::Invalid,
    ]
    .into_iter()
    .enumerate()
    {
        for project in [P::Permit, P::Deny, P::NeedsApproval, P::Unknown] {
            let mut a = fixture(&p);
            checked(&mut a.policy).plugin_trust = absent(state);
            checked(&mut a.policy).project_policy = project;
            denied(
                &a,
                BrokerReadDenial::FactUnavailable(BrokerFactKind::PluginTrust, reason),
            );
            assert_eq!(
                evaluate_broker_read(&a).audit.plugin_trust(),
                Some(absent(state))
            );
        }
    }
}
#[test]
fn every_policy_factor_and_unqualified_runtime_deny_despite_valid_review() {
    let p = package();
    for factor in [
        BrokerPolicyFactor::Package,
        BrokerPolicyFactor::InstalledGrant,
        BrokerPolicyFactor::Project,
        BrokerPolicyFactor::Owner,
        BrokerPolicyFactor::Data,
    ] {
        for state in [
            PolicyDisposition::Deny,
            PolicyDisposition::NeedsApproval,
            PolicyDisposition::Unknown,
        ] {
            let mut a = fixture(&p);
            let policy = checked(&mut a.policy);
            match factor {
                BrokerPolicyFactor::Package => policy.package_policy = state,
                BrokerPolicyFactor::InstalledGrant => policy.installed_grant = state,
                BrokerPolicyFactor::Project => policy.project_policy = state,
                BrokerPolicyFactor::Owner => policy.owner_policy = state,
                BrokerPolicyFactor::Data => policy.data_policy = state,
            }
            denied(&a, BrokerReadDenial::PolicyDenied(factor, state));
        }
    }
    for state in [RuntimeDisposition::Denied, RuntimeDisposition::Unknown] {
        let mut a = fixture(&p);
        checked(&mut a.runtime).disposition = state;
        denied(&a, BrokerReadDenial::RuntimeNotQualified);
    }
}
fn validity<'a>(a: &'a mut BrokerReadAssessment<'_>, i: usize) -> &'a mut ReviewValidity {
    match i {
        0 => &mut checked(&mut a.execution).validity,
        1 => &mut checked(&mut a.policy).validity,
        2 => &mut checked(&mut a.runtime).validity,
        3 => &mut checked(&mut a.lease).validity,
        _ => &mut checked(&mut a.revocation).validity,
    }
}
#[test]
fn all_fact_intervals_and_rollback_are_checked_with_exclusive_end() {
    let p = package();
    for (i, kind) in [
        BrokerFactKind::Execution,
        BrokerFactKind::Policy,
        BrokerFactKind::Runtime,
        BrokerFactKind::Lease,
        BrokerFactKind::Revocation,
    ]
    .into_iter()
    .enumerate()
    {
        for (start, end, reason) in [
            (150, 150, BrokerReadDenial::InvalidValidity(kind)),
            (151, 150, BrokerReadDenial::InvalidValidity(kind)),
            (151, 200, BrokerReadDenial::NotYetValid(kind)),
            (100, 150, BrokerReadDenial::Expired(kind)),
        ] {
            let mut a = fixture(&p);
            *validity(&mut a, i) = ReviewValidity {
                start_ms: start,
                end_ms: end,
            };
            denied(&a, reason);
        }
        for (start, end) in [(150, 151), (0, 151), (100, u64::MAX)] {
            let mut a = fixture(&p);
            *validity(&mut a, i) = ReviewValidity {
                start_ms: start,
                end_ms: end,
            };
            assert_eq!(eligible(&a).valid_until(), end.min(300));
        }
    }
    let mut a = fixture(&p);
    checked(&mut a.clock).last_observed_ms = 151;
    denied(&a, BrokerReadDenial::ClockRollback);
    assert_eq!(evaluate_broker_read(&a).audit.evaluated_at(), None);
    for now in [0, u64::MAX - 1] {
        let mut a = fixture(&p);
        for i in 0..5 {
            *validity(&mut a, i) = ReviewValidity {
                start_ms: now,
                end_ms: now + 1,
            };
        }
        checked(&mut a.review.approval).validity = ReviewValidity {
            start_ms: now,
            end_ms: now + 1,
        };
        checked(&mut a.review.revocation).validity = ReviewValidity {
            start_ms: now,
            end_ms: now + 1,
        };
        checked(&mut a.clock).now_ms = now;
        checked(&mut a.clock).last_observed_ms = now;
        checked(&mut a.review.clock).now_ms = now;
        checked(&mut a.review.clock).last_observed_ms = now;
        assert_eq!(eligible(&a).valid_until(), now + 1);
        checked(&mut a.clock).now_ms = now + 1;
        checked(&mut a.review.clock).now_ms = now + 1;
        denied(&a, BrokerReadDenial::Expired(BrokerFactKind::Execution));
    }
}
#[test]
fn revisions_lease_keys_and_current_policy_cannot_be_substituted() {
    use BrokerFactKind as K;
    use BrokerReadDenial as D;
    use BrokerRevisionField as R;
    let p = package();
    for (i, field) in [
        R::ExecutionRun,
        R::InputRun,
        R::PolicyPolicy,
        R::PolicyGrant,
        R::LeaseGrant,
        R::LeasePolicy,
        R::RevocationGrant,
        R::RevocationPolicy,
    ]
    .into_iter()
    .enumerate()
    {
        let mut a = fixture(&p);
        match i {
            0 => checked(&mut a.execution).run_revision = 0,
            1 => checked(&mut a.input).run_revision = 0,
            2 => checked(&mut a.policy).policy_revision = 0,
            3 => checked(&mut a.policy).grant_revision = 0,
            4 => checked(&mut a.lease).grant_revision = 0,
            5 => checked(&mut a.lease).policy_revision = 0,
            6 => checked(&mut a.revocation).grant_revision = 0,
            _ => checked(&mut a.revocation).policy_revision = 0,
        }
        denied(&a, D::InvalidRevision(field));
    }
    let mut a = fixture(&p);
    checked(&mut a.input).run_revision = 2;
    denied(&a, D::RunRevisionMismatch);
    let mut a = fixture(&p);
    a.request.lease_record_key = key(99);
    denied(&a, D::LeaseRecordMismatch);
    for (i, reason) in [
        D::GrantKeyMismatch(K::Lease),
        D::GrantKeyMismatch(K::Revocation),
        D::GrantRevisionMismatch(K::Lease),
        D::GrantRevisionMismatch(K::Revocation),
        D::PolicyRevisionMismatch(K::Lease),
        D::PolicyRevisionMismatch(K::Revocation),
    ]
    .into_iter()
    .enumerate()
    {
        let mut a = fixture(&p);
        match i {
            0 => checked(&mut a.lease).grant_key = key(99),
            1 => checked(&mut a.revocation).grant_key = key(99),
            2 => checked(&mut a.lease).grant_revision = 2,
            3 => checked(&mut a.revocation).grant_revision = 2,
            4 => checked(&mut a.lease).policy_revision = 2,
            _ => checked(&mut a.revocation).policy_revision = 2,
        }
        denied(&a, reason);
    }
    let mut a = fixture(&p);
    checked(&mut a.policy).policy_revision = 2;
    checked(&mut a.policy).plugin_trust = ReviewFact::Checked(BrokerPluginTrustDecision::Deny);
    denied(&a, D::PolicyRevisionMismatch(K::Lease));
    checked(&mut a.lease).policy_revision = 2;
    denied(&a, D::PolicyRevisionMismatch(K::Revocation));
    checked(&mut a.revocation).policy_revision = 2;
    denied(&a, D::PluginTrustDenied(BrokerPluginTrustDecision::Deny));
}
#[test]
fn lease_budget_ranges_do_not_overflow_or_claim_consumption() {
    use BrokerReadDenial as D;
    let p = package();
    for (offset, length, reason) in [
        (0, 0, D::InvalidRequestRange),
        (u64::MAX, 1, D::InvalidRequestRange),
        (0, READ_BYTES_PER_REQUEST_MAX + 1, D::ReadLimitExceeded),
        (u64::MAX, u64::MAX, D::ReadLimitExceeded),
    ] {
        let mut a = fixture(&p);
        a.request.target.offset = offset;
        a.request.target.length = length;
        denied(&a, reason);
    }
    let mut a = fixture(&p);
    a.request.target.offset = a.request.target.member.blob_length;
    denied(&a, D::InvalidRequestRange);
    let mut a = fixture(&p);
    set_member(&mut a, |m| m.blob_length = 0);
    checked(&mut a.lease).allowed_length = 0;
    denied(&a, D::InvalidLeaseRange);
    for (offset, length) in [(0, 0), (u64::MAX, 1), (0, u64::MAX)] {
        let mut a = fixture(&p);
        checked(&mut a.lease).allowed_offset = offset;
        checked(&mut a.lease).allowed_length = length;
        denied(&a, D::InvalidLeaseRange);
    }
    for (offset, length) in [(9, 16), (0, 23), (0, 8)] {
        let mut a = fixture(&p);
        checked(&mut a.lease).allowed_offset = offset;
        checked(&mut a.lease).allowed_length = length;
        denied(&a, D::LeaseRangeDenied);
    }
    let mut a = fixture(&p);
    checked(&mut a.lease).remaining_operations = 0;
    denied(&a, D::OperationBudgetExhausted);
    for bytes in [0, 15] {
        let mut a = fixture(&p);
        checked(&mut a.lease).remaining_bytes = bytes;
        denied(&a, D::ByteBudgetExhausted);
    }
    let mut a = fixture(&p);
    set_member(&mut a, |m| m.blob_length = u64::MAX);
    let l = checked(&mut a.lease);
    l.allowed_length = u64::MAX;
    l.remaining_bytes = u64::MAX;
    l.remaining_operations = u64::MAX;
    a.request.target.offset = u64::MAX - 16;
    a.request.target.length = 16;
    let before = a.lease;
    let c = eligible(&a);
    assert_eq!(c, eligible(&a));
    assert_eq!(a.lease, before);
    a.request.target.length = 17;
    denied(&a, D::InvalidRequestRange);
    let mut a = fixture(&p);
    let l = checked(&mut a.lease);
    l.remaining_operations = 1;
    l.remaining_bytes = 16;
    l.allowed_offset = 8;
    l.allowed_length = 16;
    eligible(&a);
    eligible(&a);
    assert_eq!(checked(&mut a.lease).remaining_bytes, 16);
}
#[test]
fn inactive_revoked_unknown_or_stale_state_never_allows() {
    use BrokerReadDenial as D;
    let p = package();
    for state in [RunState::Inactive, RunState::Cancelled, RunState::Unknown] {
        let mut a = fixture(&p);
        checked(&mut a.execution).run_state = state;
        denied(&a, D::RunNotActive);
    }
    for state in [Membership::Absent, Membership::Unknown] {
        let mut a = fixture(&p);
        checked(&mut a.input).membership = state;
        denied(&a, D::NotRunInput);
    }
    for state in [
        LeaseState::Revoked,
        LeaseState::Consumed,
        LeaseState::Unknown,
    ] {
        let mut a = fixture(&p);
        checked(&mut a.lease).state = state;
        denied(&a, D::LeaseNotActive);
    }
    for state in [RevocationState::Revoked, RevocationState::Unknown] {
        let mut a = fixture(&p);
        checked(&mut a.revocation).state = state;
        denied(&a, D::RevocationDenied);
    }
    let mut a = fixture(&p);
    checked(&mut a.clock).minimum_revocation_epoch = 6;
    denied(&a, D::RevocationRollback);
    let mut a = fixture(&p);
    checked(&mut a.revocation).epoch = 6;
    denied(&a, D::LeaseEpochMismatch);
    for epoch in [0, u64::MAX] {
        let mut a = fixture(&p);
        checked(&mut a.revocation).epoch = epoch;
        checked(&mut a.lease).revocation_epoch = epoch;
        checked(&mut a.clock).minimum_revocation_epoch = epoch;
        eligible(&a);
    }
}
#[test]
fn review_is_rechecked_and_missing_clock_is_not_unwrapped() {
    use BrokerReadDenial as D;
    let p = package();
    for field in 0..6 {
        let mut a = fixture(&p);
        change_identity(&mut checked(&mut a.review.approval).identity, field);
        let expected = [
            ReviewIdentityField::Manifest,
            ReviewIdentityField::Artifact,
            ReviewIdentityField::DependencyClosure,
            ReviewIdentityField::Permissions,
            ReviewIdentityField::Profile,
            ReviewIdentityField::PlatformTuple,
        ][field];
        denied(
            &a,
            D::ReviewRejected(ReviewDenial::IdentityMismatch(expected)),
        );
        let mut a = fixture(&p);
        change_identity(&mut checked(&mut a.review.requirement).identity, field);
        change_identity(&mut checked(&mut a.review.approval).identity, field);
        denied(&a, D::ReviewIdentityMismatch);
    }
    for (index, reason) in [
        ReviewDenial::Revoked,
        ReviewDenial::UnknownRevocation,
        ReviewDenial::ApprovalExpired,
        ReviewDenial::SnapshotExpired,
        ReviewDenial::ClockRollback,
        ReviewDenial::SnapshotRollback,
        ReviewDenial::PolicyMismatch,
        ReviewDenial::EvidenceMismatch,
        ReviewDenial::RevocationBindingMismatch,
        ReviewDenial::ApprovalNotYetValid,
        ReviewDenial::SnapshotNotYetValid,
        ReviewDenial::InvalidApprovalValidity,
        ReviewDenial::InvalidSnapshotValidity,
    ]
    .into_iter()
    .enumerate()
    {
        let mut a = fixture(&p);
        match index {
            0 => checked(&mut a.review.revocation).status = ReviewRevocationStatus::Revoked,
            1 => checked(&mut a.review.revocation).status = ReviewRevocationStatus::Unknown,
            2 => checked(&mut a.review.approval).validity.end_ms = 150,
            3 => checked(&mut a.review.revocation).validity.end_ms = 150,
            4 => checked(&mut a.review.clock).last_observed_ms = 151,
            5 => checked(&mut a.review.clock).minimum_snapshot_epoch = 6,
            6 => checked(&mut a.review.approval).policy_version = 2,
            7 => checked(&mut a.review.approval).evidence_digest = digest(99).sha256(),
            8 => checked(&mut a.review.revocation).approval_record_digest = digest(99).sha256(),
            9 => checked(&mut a.review.approval).validity.start_ms = 151,
            10 => checked(&mut a.review.revocation).validity.start_ms = 151,
            11 => checked(&mut a.review.approval).validity.start_ms = 400,
            _ => checked(&mut a.review.revocation).validity.start_ms = 400,
        }
        denied(&a, D::ReviewRejected(reason));
    }
    for kind in [
        ReviewFactKind::Requirement,
        ReviewFactKind::Approval,
        ReviewFactKind::Revocation,
        ReviewFactKind::Clock,
    ] {
        for state in 0..3 {
            let mut a = fixture(&p);
            match kind {
                ReviewFactKind::Requirement => a.review.requirement = absent(state),
                ReviewFactKind::Approval => a.review.approval = absent(state),
                ReviewFactKind::Revocation => a.review.revocation = absent(state),
                ReviewFactKind::Clock => a.review.clock = absent(state),
            }
            let reason = match state {
                0 => ReviewDenial::MissingFact(kind),
                1 => ReviewDenial::UnverifiedFact(kind),
                _ => ReviewDenial::InvalidFact(kind),
            };
            denied(&a, D::ReviewRejected(reason));
        }
    }
    let mut a = fixture(&p);
    checked(&mut a.review.clock).now_ms = 151;
    denied(&a, D::ReviewClockMismatch);
}
#[test]
fn manifest_permission_is_an_upper_bound_not_inferred_from_review() {
    let p = package();
    let mut a = fixture(&p);
    let wrong = digest(99).sha256();
    for i in 0..7 {
        bindings(&mut a, i).execution_identity.manifest_digest = wrong;
    }
    denied(&a, BrokerReadDenial::ManifestMismatch);
    let raw = std::str::from_utf8(p.canonical_bytes()).unwrap();
    let without = raw.replace(
        r#"[{"kind":"broker.asset.read@1","scope":"run-inputs"}]"#,
        "[]",
    );
    assert_ne!(without, raw);
    let no_permission = inspect_manifest(without.as_bytes()).unwrap();
    let a = fixture(&no_permission);
    denied(&a, BrokerReadDenial::ReadNotRequested);
}
#[test]
fn audit_preserves_only_supplied_metadata_including_on_early_denial() {
    let p = package();
    let mut a = fixture(&p);
    a.caller = ReviewFact::Missing;
    let result = evaluate_broker_read(&a);
    assert_eq!(result.audit.caller(), None);
    assert_eq!(
        result.audit.plugin_trust(),
        Some(ReviewFact::Checked(
            BrokerPluginTrustDecision::AllowReviewed
        ))
    );
    for state in 0..3 {
        let mut a = fixture(&p);
        a.clock = absent(state);
        assert_eq!(evaluate_broker_read(&a).audit.evaluated_at(), None);
    }
    let mut a = fixture(&p);
    checked(&mut a.policy).policy_revision = 0;
    let audit = evaluate_broker_read(&a).audit;
    assert_eq!(audit.plugin_trust(), None);
    assert_eq!(audit.policy_revision(), None);
    let mut a = fixture(&p);
    checked(&mut a.lease).authorization_key.binding.owner_uid = 777;
    let audit = evaluate_broker_read(&a).audit;
    assert_eq!(audit.caller().unwrap().value().owner_uid, 501);
    let mut a = fixture(&p);
    checked(&mut a.runtime).validity.start_ms = 400;
    assert_eq!(
        evaluate_broker_read(&a)
            .audit
            .runtime_qualification_digest(),
        None
    );
    for text in [
        format!("{a:?}"),
        format!("{}", a.request),
        format!("{}", checked(&mut a.caller)),
        format!("{}", checked(&mut a.policy)),
        format!("{}", evaluate_broker_read(&a).audit),
    ] {
        assert!(text.ends_with("(REDACTED)"));
        assert!(!text.contains("501"));
    }
}

#[test]
fn simultaneous_failures_preserve_stage_and_nested_fact_precedence() {
    use BrokerFactKind as K;
    use BrokerReadDenial as D;
    type Mutation = fn(&mut BrokerReadAssessment<'_>);
    let stages: [(Mutation, D); 8] = [
        (
            |a| a.caller = ReviewFact::Missing,
            D::FactUnavailable(K::Caller, UnavailableFact::Missing),
        ),
        (
            |a| checked(&mut a.execution).run_revision = 0,
            D::InvalidRevision(BrokerRevisionField::ExecutionRun),
        ),
        (
            |a| checked(&mut a.execution).binding.channel_key = key(99),
            D::BindingMismatch(K::Execution, BrokerBindingField::Channel),
        ),
        (
            |a| checked(&mut a.clock).last_observed_ms = 151,
            D::ClockRollback,
        ),
        (
            |a| checked(&mut a.execution).run_state = RunState::Cancelled,
            D::RunNotActive,
        ),
        (
            |a| checked(&mut a.review.revocation).status = ReviewRevocationStatus::Revoked,
            D::ReviewRejected(ReviewDenial::Revoked),
        ),
        (
            |a| {
                checked(&mut a.policy).plugin_trust =
                    ReviewFact::Checked(BrokerPluginTrustDecision::Deny)
            },
            D::PluginTrustDenied(BrokerPluginTrustDecision::Deny),
        ),
        (
            |a| checked(&mut a.lease).remaining_operations = 0,
            D::OperationBudgetExhausted,
        ),
    ];
    let p = package();
    for pair in stages.windows(2) {
        let mut a = fixture(&p);
        (pair[0].0)(&mut a);
        (pair[1].0)(&mut a);
        denied(&a, pair[0].1);
    }
    let mut a = fixture(&p);
    for (mutate, _) in stages {
        mutate(&mut a);
    }
    denied(&a, stages[0].1);
    let facts: [(Mutation, K); 9] = [
        (|a| a.caller = ReviewFact::Missing, K::Caller),
        (|a| a.execution = ReviewFact::Missing, K::Execution),
        (|a| a.input = ReviewFact::Missing, K::Input),
        (|a| a.policy = ReviewFact::Missing, K::Policy),
        (
            |a| checked(&mut a.policy).plugin_trust = ReviewFact::Missing,
            K::PluginTrust,
        ),
        (|a| a.runtime = ReviewFact::Missing, K::Runtime),
        (|a| a.lease = ReviewFact::Missing, K::Lease),
        (|a| a.revocation = ReviewFact::Missing, K::Revocation),
        (|a| a.clock = ReviewFact::Missing, K::Clock),
    ];
    for pair in facts.windows(2) {
        let mut a = fixture(&p);
        (pair[1].0)(&mut a);
        (pair[0].0)(&mut a);
        denied(&a, D::FactUnavailable(pair[0].1, UnavailableFact::Missing));
    }
    let mut a = fixture(&p);
    checked(&mut a.lease).allowed_length = 0;
    a.request.target.length = u64::MAX;
    denied(&a, D::InvalidLeaseRange);
    let mut a = fixture(&p);
    a.request.target.offset = u64::MAX;
    a.request.target.length = u64::MAX;
    denied(&a, D::ReadLimitExceeded);
    let mut a = fixture(&p);
    checked(&mut a.runtime).disposition = RuntimeDisposition::Unknown;
    checked(&mut a.policy).plugin_trust = ReviewFact::Checked(BrokerPluginTrustDecision::Deny);
    denied(&a, D::RuntimeNotQualified);
    let mut a = fixture(&p);
    checked(&mut a.lease).remaining_operations = 0;
    checked(&mut a.lease).remaining_bytes = 0;
    denied(&a, D::OperationBudgetExhausted);
}
