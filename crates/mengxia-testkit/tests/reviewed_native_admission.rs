use mengxia_plugin_security::{
    ReviewApproval, ReviewAssessment, ReviewClock, ReviewDenial, ReviewFact, ReviewFactKind,
    ReviewIdentity, ReviewIdentityField, ReviewRequirement, ReviewRevocationSnapshot,
    ReviewRevocationStatus, ReviewValidity, evaluate_review_eligibility,
};

fn fixture() -> ReviewAssessment {
    let identity = ReviewIdentity {
        manifest_digest: "11".repeat(32).parse().unwrap(),
        artifact_digest: "22".repeat(32).parse().unwrap(),
        dependency_closure_digest: "33".repeat(32).parse().unwrap(),
        permissions_digest: "44".repeat(32).parse().unwrap(),
        profile_digest: "55".repeat(32).parse().unwrap(),
        platform_tuple_digest: "66".repeat(32).parse().unwrap(),
    };
    let evidence_digest = "77".repeat(32).parse().unwrap();
    let record_digest = "88".repeat(32).parse().unwrap();
    ReviewAssessment {
        requirement: ReviewFact::Checked(ReviewRequirement {
            identity,
            policy_version: 1,
            evidence_digest,
        }),
        approval: ReviewFact::Checked(ReviewApproval {
            identity,
            policy_version: 1,
            evidence_digest,
            record_digest,
            validity: ReviewValidity {
                start_ms: 100,
                end_ms: 400,
            },
        }),
        revocation: ReviewFact::Checked(ReviewRevocationSnapshot {
            approval_record_digest: record_digest,
            status: ReviewRevocationStatus::Clear,
            epoch: 5,
            validity: ReviewValidity {
                start_ms: 120,
                end_ms: 300,
            },
        }),
        clock: ReviewFact::Checked(ReviewClock {
            now_ms: 150,
            last_observed_ms: 140,
            minimum_snapshot_epoch: 5,
        }),
    }
}

fn value<T>(fact: &mut ReviewFact<T>) -> &mut T {
    match fact {
        ReviewFact::Checked(value) => value,
        _ => panic!("fixture must contain a checked fact"),
    }
}

#[test]
fn exact_matches_return_only_a_bound_candidate_with_the_earliest_expiry() {
    let mut input = fixture();
    let approval = *value(&mut input.approval);
    let result = evaluate_review_eligibility(&input).unwrap();
    assert_eq!(result.identity(), approval.identity);
    assert_eq!(result.approval_record_digest(), approval.record_digest);
    assert_eq!(result.evidence_digest(), approval.evidence_digest);
    assert_eq!(result.policy_version(), 1);
    assert_eq!(result.evaluated_at_ms(), 150);
    assert_eq!(result.valid_until_ms(), 300);
    assert_eq!(result.snapshot_epoch(), 5);
    value(&mut input.approval).validity.end_ms = 200;
    assert_eq!(
        evaluate_review_eligibility(&input)
            .unwrap()
            .valid_until_ms(),
        200
    );
    // A prior candidate does not make a fresh evaluation pass after revocation.
    value(&mut input.revocation).status = ReviewRevocationStatus::Revoked;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::Revoked)
    );
}

#[test]
fn missing_unverified_and_invalid_facts_never_default_to_approval() {
    for kind in [
        ReviewFactKind::Requirement,
        ReviewFactKind::Approval,
        ReviewFactKind::Revocation,
        ReviewFactKind::Clock,
    ] {
        for state in 0..3 {
            fn replace<T>(fact: &mut ReviewFact<T>, state: u8) {
                *fact = match state {
                    0 => ReviewFact::Missing,
                    1 => ReviewFact::Unverified,
                    _ => ReviewFact::Invalid,
                };
            }
            let mut input = fixture();
            match kind {
                ReviewFactKind::Requirement => replace(&mut input.requirement, state),
                ReviewFactKind::Approval => replace(&mut input.approval, state),
                ReviewFactKind::Revocation => replace(&mut input.revocation, state),
                ReviewFactKind::Clock => replace(&mut input.clock, state),
            }
            let reason = match state {
                0 => ReviewDenial::MissingFact(kind),
                1 => ReviewDenial::UnverifiedFact(kind),
                _ => ReviewDenial::InvalidFact(kind),
            };
            assert_eq!(evaluate_review_eligibility(&input), Err(reason));
        }
    }
}

fn change_identity(identity: &mut ReviewIdentity, field: ReviewIdentityField) {
    let changed = "ff".repeat(32).parse().unwrap();
    match field {
        ReviewIdentityField::Manifest => identity.manifest_digest = changed,
        ReviewIdentityField::Artifact => identity.artifact_digest = changed,
        ReviewIdentityField::DependencyClosure => identity.dependency_closure_digest = changed,
        ReviewIdentityField::Permissions => identity.permissions_digest = changed,
        ReviewIdentityField::Profile => identity.profile_digest = changed,
        ReviewIdentityField::PlatformTuple => identity.platform_tuple_digest = changed,
    }
}

#[test]
fn each_identity_dimension_is_bound_on_both_sides_not_just_the_manifest() {
    for field in [
        ReviewIdentityField::Manifest,
        ReviewIdentityField::Artifact,
        ReviewIdentityField::DependencyClosure,
        ReviewIdentityField::Permissions,
        ReviewIdentityField::Profile,
        ReviewIdentityField::PlatformTuple,
    ] {
        for change_approval in [true, false] {
            let mut input = fixture();
            if change_approval {
                change_identity(&mut value(&mut input.approval).identity, field);
            } else {
                change_identity(&mut value(&mut input.requirement).identity, field);
            }
            assert_eq!(
                evaluate_review_eligibility(&input),
                Err(ReviewDenial::IdentityMismatch(field))
            );
        }
    }
}

#[test]
fn policy_evidence_and_revocation_record_cannot_be_substituted() {
    let mut input = fixture();
    value(&mut input.approval).policy_version = 2;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::PolicyMismatch)
    );
    let mut input = fixture();
    value(&mut input.approval).evidence_digest = "ff".repeat(32).parse().unwrap();
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::EvidenceMismatch)
    );
    for change_approval in [true, false] {
        let mut input = fixture();
        let different = "ff".repeat(32).parse().unwrap();
        if change_approval {
            value(&mut input.approval).record_digest = different;
        } else {
            value(&mut input.revocation).approval_record_digest = different;
        }
        assert_eq!(
            evaluate_review_eligibility(&input),
            Err(ReviewDenial::RevocationBindingMismatch)
        );
    }
}

#[test]
fn validity_intervals_reject_empty_reversed_and_exclusive_end_for_both_records() {
    for approval in [true, false] {
        for (start, end) in [(0, 0), (1, 1), (2, 1), (u64::MAX, 0), (u64::MAX, u64::MAX)] {
            let mut input = fixture();
            let interval = ReviewValidity {
                start_ms: start,
                end_ms: end,
            };
            let error = if approval {
                value(&mut input.approval).validity = interval;
                ReviewDenial::InvalidApprovalValidity
            } else {
                value(&mut input.revocation).validity = interval;
                ReviewDenial::InvalidSnapshotValidity
            };
            assert_eq!(evaluate_review_eligibility(&input), Err(error));
        }
        for now in [99, 100, 199, 200] {
            let mut input = fixture();
            let wide = ReviewValidity {
                start_ms: 0,
                end_ms: u64::MAX,
            };
            let tested = ReviewValidity {
                start_ms: 100,
                end_ms: 200,
            };
            value(&mut input.approval).validity = if approval { tested } else { wide };
            value(&mut input.revocation).validity = if approval { wide } else { tested };
            value(&mut input.clock).now_ms = now;
            value(&mut input.clock).last_observed_ms = 0;
            let result = evaluate_review_eligibility(&input);
            match now {
                99 => assert_eq!(
                    result,
                    Err(if approval {
                        ReviewDenial::ApprovalNotYetValid
                    } else {
                        ReviewDenial::SnapshotNotYetValid
                    })
                ),
                200 => assert_eq!(
                    result,
                    Err(if approval {
                        ReviewDenial::ApprovalExpired
                    } else {
                        ReviewDenial::SnapshotExpired
                    })
                ),
                _ => assert_eq!(result.unwrap().valid_until_ms(), 200),
            }
        }
    }
}

#[test]
fn unknown_revocation_and_clock_or_snapshot_rollback_fail_closed() {
    let mut input = fixture();
    value(&mut input.revocation).status = ReviewRevocationStatus::Unknown;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::UnknownRevocation)
    );
    let mut input = fixture();
    value(&mut input.clock).last_observed_ms = 151;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::ClockRollback)
    );
    let mut input = fixture();
    value(&mut input.clock).minimum_snapshot_epoch = 6;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::SnapshotRollback)
    );
    value(&mut input.revocation).epoch = 6;
    assert!(evaluate_review_eligibility(&input).is_ok());
    value(&mut input.revocation).epoch = u64::MAX;
    assert!(evaluate_review_eligibility(&input).is_ok());
}

#[test]
fn integer_extremes_and_zero_values_are_not_sentinels_or_arithmetic_overflow() {
    for (now, end, version) in [(0, 1, 0), (u64::MAX - 1, u64::MAX, u64::MAX)] {
        let mut input = fixture();
        let validity = ReviewValidity {
            start_ms: now,
            end_ms: end,
        };
        value(&mut input.approval).validity = validity;
        value(&mut input.revocation).validity = validity;
        value(&mut input.approval).policy_version = version;
        value(&mut input.requirement).policy_version = version;
        value(&mut input.clock).now_ms = now;
        value(&mut input.clock).last_observed_ms = now;
        value(&mut input.clock).minimum_snapshot_epoch = u64::MAX;
        value(&mut input.revocation).epoch = u64::MAX;
        value(&mut input.approval).identity.artifact_digest = "00".repeat(32).parse().unwrap();
        value(&mut input.requirement).identity.artifact_digest = "00".repeat(32).parse().unwrap();
        let candidate = evaluate_review_eligibility(&input).unwrap();
        assert_eq!(candidate.evaluated_at_ms(), now);
        assert_eq!(candidate.valid_until_ms(), end);
        value(&mut input.clock).now_ms = end;
        assert_eq!(
            evaluate_review_eligibility(&input),
            Err(ReviewDenial::ApprovalExpired)
        );
    }
}

#[test]
fn simultaneous_failures_have_deterministic_stage_and_identity_precedence() {
    let mut input = fixture();
    input.requirement = ReviewFact::Missing;
    input.approval = ReviewFact::Unverified;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::MissingFact(ReviewFactKind::Requirement))
    );
    let mut input = fixture();
    value(&mut input.approval).validity.end_ms = 0;
    value(&mut input.revocation).status = ReviewRevocationStatus::Unknown;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::InvalidApprovalValidity)
    );
    let mut input = fixture();
    value(&mut input.revocation).status = ReviewRevocationStatus::Revoked;
    value(&mut input.clock).now_ms = 500;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::Revoked)
    );
    value(&mut input.revocation).status = ReviewRevocationStatus::Clear;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::ApprovalExpired)
    );
    let mut input = fixture();
    value(&mut input.clock).last_observed_ms = 151;
    value(&mut input.revocation).epoch = 0;
    change_identity(
        &mut value(&mut input.approval).identity,
        ReviewIdentityField::Artifact,
    );
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::ClockRollback)
    );
    value(&mut input.clock).last_observed_ms = 150;
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::SnapshotRollback)
    );
    value(&mut input.revocation).epoch = 5;
    change_identity(
        &mut value(&mut input.approval).identity,
        ReviewIdentityField::Manifest,
    );
    assert_eq!(
        evaluate_review_eligibility(&input),
        Err(ReviewDenial::IdentityMismatch(
            ReviewIdentityField::Manifest
        ))
    );
}
