use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::str::FromStr;
use std::thread;

use mengxia_types::{
    ErrorCode, Id, IdGenerationError, RevisionNo, RevisionOverflow, Sha256Digest, Timestamp,
    ValueError,
};
use proptest::prelude::*;

struct Asset;

fn assert_value_traits<T>()
where
    T: Clone + Copy + Eq + Ord + Hash + Debug + Display + FromStr,
{
}

#[test]
fn foundation_values_expose_the_accepted_trait_set() {
    assert_value_traits::<Id<Asset>>();
    assert_value_traits::<Sha256Digest>();
    assert_value_traits::<Timestamp>();
    assert_value_traits::<RevisionNo>();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn typed_id_bytes_and_text_round_trip(mut bytes in any::<[u8; 16]>()) {
        bytes[6] = (bytes[6] & 0x0f) | 0x70;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let id = Id::<Asset>::from_bytes(bytes).expect("forced RFC UUIDv7 bytes are valid");
        let text = id.to_string();
        prop_assert_eq!(text.len(), 36);
        let lowercase = text.to_ascii_lowercase();
        prop_assert_eq!(&text, &lowercase);
        prop_assert_eq!(text.parse::<Id<Asset>>().expect("canonical ID parses"), id);
        prop_assert_eq!(id.to_bytes(), bytes);
    }

    #[test]
    fn digest_bytes_and_text_round_trip(bytes in any::<[u8; 32]>()) {
        let digest = Sha256Digest::from_bytes(bytes);
        let text = digest.to_string();
        prop_assert_eq!(text.len(), 64);
        prop_assert!(text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        prop_assert_eq!(text.parse::<Sha256Digest>().expect("canonical digest parses"), digest);
        prop_assert_eq!(digest.to_bytes(), bytes);
    }

    #[test]
    fn timestamp_components_and_text_round_trip(
        seconds in -62_135_596_800_i64..=253_402_300_799_i64,
        nanos in 0_u32..=999_999_999_u32,
    ) {
        let timestamp = Timestamp::from_unix_seconds_nanos(seconds, nanos)
            .expect("accepted timestamp components are in range");
        prop_assert_eq!(timestamp.unix_seconds(), seconds);
        prop_assert_eq!(timestamp.subsec_nanoseconds(), nanos);
        let text = timestamp.to_string();
        prop_assert_eq!(text.parse::<Timestamp>().expect("canonical timestamp parses"), timestamp);
    }

    #[test]
    fn revision_text_round_trips(value in any::<u64>()) {
        let revision = RevisionNo::new(value);
        prop_assert_eq!(revision.to_string().parse::<RevisionNo>().expect("canonical revision parses"), revision);
    }
}

#[test]
fn generated_ids_are_valid_and_do_not_collide_in_parallel_sample() {
    const THREADS: usize = 8;
    const IDS_PER_THREAD: usize = 512;
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            thread::spawn(|| {
                (0..IDS_PER_THREAD)
                    .map(|_| {
                        Id::<Asset>::try_new()
                            .expect("test host supplies valid time and entropy")
                            .to_bytes()
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();

    let ids: HashSet<_> = handles
        .into_iter()
        .flat_map(|handle| handle.join().expect("generation thread must finish"))
        .collect();
    assert_eq!(ids.len(), THREADS * IDS_PER_THREAD);
    for bytes in ids {
        assert!(Id::<Asset>::from_bytes(bytes).is_ok());
    }
}

#[test]
fn typed_id_rejects_noncanonical_wrong_variant_and_wrong_version_inputs() {
    let canonical = "01890f3e-7a5b-7c4d-8e9f-1029384756ab";
    assert_eq!(
        canonical.parse::<Id<Asset>>().map(Id::to_bytes),
        Id::<Asset>::from_bytes([
            0x01, 0x89, 0x0f, 0x3e, 0x7a, 0x5b, 0x7c, 0x4d, 0x8e, 0x9f, 0x10, 0x29, 0x38, 0x47,
            0x56, 0xab,
        ])
        .map(Id::to_bytes)
    );

    for invalid in [
        "",
        "01890f3e-7a5b-7c4d-8e9f-1029384756a",
        "01890f3e-7a5b-7c4d-8e9f-1029384756abc",
        "01890F3E-7A5B-7C4D-8E9F-1029384756AB",
        "01890f3e7a5b7c4d8e9f1029384756ab",
        "{01890f3e-7a5b-7c4d-8e9f-1029384756ab}",
        "urn:uuid:01890f3e-7a5b-7c4d-8e9f-1029384756ab",
        " 01890f3e-7a5b-7c4d-8e9f-1029384756ab",
        "00000000-0000-0000-0000-000000000000",
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "aaaaaaaa-aaaa-7aaa-0aaa-aaaaaaaaaaaa",
        "aaaaaaaa-aaaa-7aaa-caaa-aaaaaaaaaaaa",
        "aaaaaaaa-aaaa-7aaa-éaaaaaaaaaaaaaa",
    ] {
        assert_eq!(invalid.parse::<Id<Asset>>(), Err(ValueError::InvalidId));
    }

    let mut non_v7 = [0xaa; 16];
    non_v7[6] = 0x4a;
    non_v7[8] = 0x8a;
    assert_eq!(Id::<Asset>::from_bytes(non_v7), Err(ValueError::InvalidId));
    let mut non_rfc = [0xaa; 16];
    non_rfc[6] = 0x7a;
    non_rfc[8] = 0x0a;
    assert_eq!(Id::<Asset>::from_bytes(non_rfc), Err(ValueError::InvalidId));
}

#[test]
fn digest_rejects_every_noncanonical_boundary_form() {
    let lowercase = "ab".repeat(32);
    assert!(lowercase.parse::<Sha256Digest>().is_ok());
    for invalid in [
        "a".repeat(63),
        "a".repeat(65),
        "AB".repeat(32),
        "aB".repeat(32),
        format!("0x{lowercase}"),
        format!("{}é", "a".repeat(62)),
        format!("{}g", "a".repeat(63)),
        format!(" {lowercase}"),
    ] {
        assert_eq!(
            invalid.parse::<Sha256Digest>(),
            Err(ValueError::InvalidDigest)
        );
    }
    assert_eq!(
        Sha256Digest::from_bytes([0; 32]).to_string(),
        "0".repeat(64)
    );
}

#[test]
fn timestamp_enforces_range_utc_and_unique_shortest_rfc3339() {
    let minimum = Timestamp::from_unix_seconds_nanos(-62_135_596_800, 0)
        .expect("year 0001 boundary is accepted");
    assert_eq!(minimum.to_string(), "0001-01-01T00:00:00Z");
    let maximum = Timestamp::from_unix_seconds_nanos(253_402_300_799, 999_999_999)
        .expect("year 9999 boundary is accepted");
    assert_eq!(maximum.to_string(), "9999-12-31T23:59:59.999999999Z");
    assert_eq!(
        "1970-01-01T00:00:00.1Z"
            .parse::<Timestamp>()
            .expect("shortest fraction parses")
            .subsec_nanoseconds(),
        100_000_000
    );

    assert_eq!(
        Timestamp::from_unix_seconds_nanos(-62_135_596_801, 0),
        Err(ValueError::InvalidTimestamp)
    );
    assert_eq!(
        Timestamp::from_unix_seconds_nanos(253_402_300_800, 0),
        Err(ValueError::InvalidTimestamp)
    );
    assert_eq!(
        Timestamp::from_unix_seconds_nanos(0, 1_000_000_000),
        Err(ValueError::InvalidTimestamp)
    );

    for invalid in [
        "1970-01-01 00:00:00Z",
        "1970-01-01T00:00:00z",
        "1970-01-01T00:00:00+00:00",
        "1970-01-01T00:00:00.10Z",
        "1970-01-01T00:00:00.000000000Z",
        "1970-01-01T00:00:00.1234567890Z",
        "2016-12-31T23:59:60Z",
        "0000-01-01T00:00:00Z",
        "1970-01-01T00:00:00Zextra",
        "1970-01-01T00:00:0éZ",
    ] {
        assert_eq!(
            invalid.parse::<Timestamp>(),
            Err(ValueError::InvalidTimestamp)
        );
    }
}

#[test]
fn revision_enforces_canonical_decimal_and_checked_exhaustion() {
    assert_eq!(RevisionNo::INITIAL.get(), 0);
    assert_eq!(RevisionNo::INITIAL.checked_next(), Ok(RevisionNo::new(1)));
    assert_eq!(
        RevisionNo::new(u64::MAX).checked_next(),
        Err(RevisionOverflow)
    );
    assert_eq!(
        u64::MAX.to_string().parse::<RevisionNo>(),
        Ok(RevisionNo::new(u64::MAX))
    );

    for invalid in [
        "",
        "00",
        "01",
        "+1",
        "-1",
        " 1",
        "1 ",
        "18446744073709551616",
        "111111111111111111111",
        "é111111111111111111",
    ] {
        assert_eq!(
            invalid.parse::<RevisionNo>(),
            Err(ValueError::InvalidRevision)
        );
    }
}

#[test]
fn all_error_codes_have_one_exact_round_trip() {
    let codes = [
        ErrorCode::ValidationError,
        ErrorCode::AuthenticationError,
        ErrorCode::AuthorizationDenied,
        ErrorCode::NotFound,
        ErrorCode::Conflict,
        ErrorCode::InvalidTransition,
        ErrorCode::SourceModifiedDuringIngest,
        ErrorCode::StorageIoError,
        ErrorCode::StorageCorruption,
        ErrorCode::ProviderValidation,
        ErrorCode::InvalidCredential,
        ErrorCode::ProviderRateLimited,
        ErrorCode::ProviderTimeout,
        ErrorCode::ProviderUnavailable,
        ErrorCode::SubmissionUnknown,
        ErrorCode::PluginProtocolViolation,
        ErrorCode::SandboxUnavailable,
        ErrorCode::PluginRevoked,
        ErrorCode::Backpressure,
        ErrorCode::InternalError,
        ErrorCode::CommandInProgress,
        ErrorCode::AdminAuthUnavailable,
        ErrorCode::UnsupportedCapability,
        ErrorCode::IdGenerationUnavailable,
        ErrorCode::RevisionExhausted,
    ];

    let unique: HashSet<_> = codes.iter().map(|code| code.as_str()).collect();
    assert_eq!(unique.len(), codes.len());
    for code in codes {
        assert_eq!(code.to_string(), code.as_str());
        assert_eq!(code.as_str().parse::<ErrorCode>(), Ok(code));
    }
    for invalid in [
        "",
        "validation_error",
        "VALIDATION_ERROR ",
        "UNKNOWN",
        "VALIDATIéN_ERROR",
    ] {
        assert_eq!(
            invalid.parse::<ErrorCode>(),
            Err(ValueError::UnknownErrorCode)
        );
    }
    assert_eq!(
        "A".repeat(30).parse::<ErrorCode>(),
        Err(ValueError::UnknownErrorCode)
    );
}

#[test]
fn public_errors_are_static_and_do_not_retain_rejected_input() {
    let canary = "secret=/Users/example/.ssh/id_ed25519?token=signed-url";
    let errors = [
        canary
            .parse::<Id<Asset>>()
            .expect_err("canary is not an ID"),
        canary
            .parse::<Sha256Digest>()
            .expect_err("canary is not a digest"),
        canary
            .parse::<Timestamp>()
            .expect_err("canary is not a timestamp"),
        canary
            .parse::<RevisionNo>()
            .expect_err("canary is not a revision"),
        canary
            .parse::<ErrorCode>()
            .expect_err("canary is not an error code"),
    ];
    let rendered = format!("{errors:?} {}", errors[0]);
    assert!(!rendered.contains(canary));

    assert_eq!(ValueError::InvalidId.to_string(), "invalid typed UUIDv7");
    assert_eq!(
        ValueError::InvalidDigest.to_string(),
        "invalid SHA-256 digest"
    );
    assert_eq!(
        ValueError::InvalidTimestamp.to_string(),
        "invalid timestamp"
    );
    assert_eq!(
        ValueError::InvalidRevision.to_string(),
        "invalid revision number"
    );
    assert_eq!(
        ValueError::UnknownErrorCode.to_string(),
        "unknown error code"
    );
    assert_eq!(
        IdGenerationError::ClockBeforeUnixEpoch.to_string(),
        "system clock is before the Unix epoch"
    );
    assert_eq!(
        IdGenerationError::TimestampOutOfRange.to_string(),
        "system clock is outside the UUIDv7 range"
    );
    assert_eq!(
        IdGenerationError::EntropyUnavailable.to_string(),
        "operating-system entropy is unavailable"
    );
    assert_eq!(RevisionOverflow.to_string(), "revision number is exhausted");
}
