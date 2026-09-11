use std::collections::BTreeSet;
use std::fmt;

use mengxia_types::{Id, RevisionNo, Sha256Digest};

pub enum Project {}
pub enum ProjectSpecRevision {}
pub enum Subject {}
pub enum WorkItem {}
pub enum WorkRevision {}
pub enum Take {}
pub enum Relationship {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CreativeError {
    InvalidValue,
    InvalidTransition,
    Conflict,
    RevisionExhausted,
}

impl fmt::Display for CreativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidValue => "creative value validation failed",
            Self::InvalidTransition => "creative transition is invalid",
            Self::Conflict => "creative revision conflict",
            Self::RevisionExhausted => "creative revision is exhausted",
        })
    }
}

impl std::error::Error for CreativeError {}

fn validate_text(value: String, maximum: usize) -> Result<String, CreativeError> {
    if value.is_empty()
        || value.len() > maximum
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err(CreativeError::InvalidValue)
    } else {
        Ok(value)
    }
}

macro_rules! bounded_text {
    ($name:ident, $maximum:expr) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, CreativeError> {
                validate_text(value.into(), $maximum).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

bounded_text!(ProjectName, 255);
bounded_text!(SubjectName, 255);
bounded_text!(WorkCode, 64);
bounded_text!(TakeReason, 1024);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SubjectKind(String);

impl SubjectKind {
    pub fn new(value: impl Into<String>) -> Result<Self, CreativeError> {
        let value = value.into();
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 64
            || !bytes[0].is_ascii_lowercase()
            || !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(CreativeError::InvalidValue);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resolution {
    width: u16,
    height: u16,
}

impl Resolution {
    pub fn new(width: u32, height: u32) -> Result<Self, CreativeError> {
        Ok(Self {
            width: u16::try_from(width).map_err(|_| CreativeError::InvalidValue)?,
            height: u16::try_from(height).map_err(|_| CreativeError::InvalidValue)?,
        })
        .and_then(|value| {
            if value.width == 0 || value.height == 0 {
                Err(CreativeError::InvalidValue)
            } else {
                Ok(value)
            }
        })
    }

    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositiveRatio {
    numerator: u32,
    denominator: u32,
}

impl PositiveRatio {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, CreativeError> {
        if numerator == 0 || denominator == 0 || gcd(numerator, denominator) != 1 {
            return Err(CreativeError::InvalidValue);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    #[must_use]
    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> u32 {
        self.denominator
    }
}

const fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalJson {
    bytes: Vec<u8>,
    digest: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSpecification {
    resolution: Option<Resolution>,
    frame_rate: Option<PositiveRatio>,
    aspect_ratio: Option<PositiveRatio>,
    color_policy: CanonicalJson,
    audio_policy: CanonicalJson,
    quality_policy: CanonicalJson,
    privacy_policy: CanonicalJson,
    policy_digest: Sha256Digest,
}

impl ProjectSpecification {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        resolution: Option<Resolution>,
        frame_rate: Option<PositiveRatio>,
        aspect_ratio: Option<PositiveRatio>,
        color_policy: CanonicalJson,
        audio_policy: CanonicalJson,
        quality_policy: CanonicalJson,
        privacy_policy: CanonicalJson,
        policy_digest: Sha256Digest,
    ) -> Self {
        Self {
            resolution,
            frame_rate,
            aspect_ratio,
            color_policy,
            audio_policy,
            quality_policy,
            privacy_policy,
            policy_digest,
        }
    }

    #[must_use]
    pub const fn resolution(&self) -> Option<Resolution> {
        self.resolution
    }
    #[must_use]
    pub const fn frame_rate(&self) -> Option<PositiveRatio> {
        self.frame_rate
    }
    #[must_use]
    pub const fn aspect_ratio(&self) -> Option<PositiveRatio> {
        self.aspect_ratio
    }
    #[must_use]
    pub const fn color_policy(&self) -> &CanonicalJson {
        &self.color_policy
    }
    #[must_use]
    pub const fn audio_policy(&self) -> &CanonicalJson {
        &self.audio_policy
    }
    #[must_use]
    pub const fn quality_policy(&self) -> &CanonicalJson {
        &self.quality_policy
    }
    #[must_use]
    pub const fn privacy_policy(&self) -> &CanonicalJson {
        &self.privacy_policy
    }
    #[must_use]
    pub const fn policy_digest(&self) -> Sha256Digest {
        self.policy_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkSpecification {
    json: CanonicalJson,
    subject_ids: Vec<Id<Subject>>,
    asset_ids: Vec<Id<crate::Asset>>,
}

impl WorkSpecification {
    pub fn new(
        json: CanonicalJson,
        subject_ids: impl IntoIterator<Item = Id<Subject>>,
        asset_ids: impl IntoIterator<Item = Id<crate::Asset>>,
    ) -> Result<Self, CreativeError> {
        Ok(Self {
            json,
            subject_ids: validate_relationship_set(subject_ids, 64)?,
            asset_ids: validate_relationship_set(asset_ids, 64)?,
        })
    }

    #[must_use]
    pub const fn json(&self) -> &CanonicalJson {
        &self.json
    }
    #[must_use]
    pub fn subject_ids(&self) -> &[Id<Subject>] {
        &self.subject_ids
    }
    #[must_use]
    pub fn asset_ids(&self) -> &[Id<crate::Asset>] {
        &self.asset_ids
    }
}

impl CanonicalJson {
    /// Trusted seam for the app-layer bounded, duplicate-rejecting JSON parser.
    #[doc(hidden)]
    pub fn __from_validated_object(
        bytes: Vec<u8>,
        digest: Sha256Digest,
        maximum_bytes: usize,
    ) -> Result<Self, CreativeError> {
        if !(2..=maximum_bytes).contains(&bytes.len())
            || bytes.first() != Some(&b'{')
            || bytes.last() != Some(&b'}')
        {
            return Err(CreativeError::InvalidValue);
        }
        Ok(Self { bytes, digest })
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn digest(&self) -> Sha256Digest {
        self.digest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkKind {
    Scene,
    Shot,
}

impl WorkKind {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Scene => 1,
            Self::Shot => 2,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scene => "SCENE",
            Self::Shot => "SHOT",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectTrust {
    Untrusted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TakeState {
    Candidate,
    Shortlisted,
    Selected,
    Approved,
    Rejected,
    Superseded,
}

impl TakeState {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Candidate => 1,
            Self::Shortlisted => 2,
            Self::Selected => 3,
            Self::Approved => 4,
            Self::Rejected => 5,
            Self::Superseded => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "CANDIDATE",
            Self::Shortlisted => "SHORTLISTED",
            Self::Selected => "SELECTED",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Superseded => "SUPERSEDED",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CreativeError> {
        match value {
            "CANDIDATE" => Ok(Self::Candidate),
            "SHORTLISTED" => Ok(Self::Shortlisted),
            "SELECTED" => Ok(Self::Selected),
            "APPROVED" => Ok(Self::Approved),
            "REJECTED" => Ok(Self::Rejected),
            "SUPERSEDED" => Ok(Self::Superseded),
            _ => Err(CreativeError::InvalidValue),
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Approved | Self::Rejected | Self::Superseded)
    }

    pub const fn transition(self, transition: TakeTransition) -> Result<Self, CreativeError> {
        match (self, transition) {
            (Self::Candidate, TakeTransition::Shortlist) => Ok(Self::Shortlisted),
            (Self::Candidate | Self::Shortlisted, TakeTransition::Select) => Ok(Self::Selected),
            (Self::Selected, TakeTransition::Approve) => Ok(Self::Approved),
            (Self::Candidate | Self::Shortlisted | Self::Selected, TakeTransition::Reject) => {
                Ok(Self::Rejected)
            }
            (Self::Candidate | Self::Shortlisted | Self::Selected, TakeTransition::Supersede) => {
                Ok(Self::Superseded)
            }
            _ => Err(CreativeError::InvalidTransition),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TakeTransition {
    Shortlist,
    Select,
    Approve,
    Reject,
    Supersede,
}

impl TakeTransition {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Shortlist => 1,
            Self::Select => 2,
            Self::Approve => 3,
            Self::Reject => 4,
            Self::Supersede => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationshipKind {
    WorkSubject,
    WorkAsset,
    TakeReopens,
    TakeSupersedes,
}

impl RelationshipKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkSubject => "WORK_SUBJECT",
            Self::WorkAsset => "WORK_ASSET",
            Self::TakeReopens => "TAKE_REOPENS",
            Self::TakeSupersedes => "TAKE_SUPERSEDES",
        }
    }
}

pub fn validate_relationship_set<T>(
    values: impl IntoIterator<Item = Id<T>>,
    maximum: usize,
) -> Result<Vec<Id<T>>, CreativeError> {
    let values: Vec<_> = values.into_iter().collect();
    let set: BTreeSet<_> = values.iter().copied().collect();
    if set.len() != values.len() || set.len() > maximum {
        return Err(CreativeError::InvalidValue);
    }
    Ok(set.into_iter().collect())
}

pub fn next_revision(current: RevisionNo) -> Result<RevisionNo, CreativeError> {
    current
        .get()
        .checked_add(1)
        .map(RevisionNo::new)
        .ok_or(CreativeError::RevisionExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_text_ratio_and_resolution_reject_noncanonical_values() {
        assert!(ProjectName::new(" Project").is_err());
        assert!(ProjectName::new("Project\n").is_err());
        assert!(ProjectName::new("P".repeat(256)).is_err());
        assert_eq!(ProjectName::new("Project").unwrap().as_str(), "Project");
        assert!(PositiveRatio::new(24, 2).is_err());
        assert!(PositiveRatio::new(0, 1).is_err());
        assert_eq!(PositiveRatio::new(24, 1).unwrap().numerator(), 24);
        assert!(Resolution::new(0, 1).is_err());
        assert!(Resolution::new(65_536, 1).is_err());
    }

    #[test]
    fn take_state_machine_has_no_terminal_or_backward_transition() {
        assert_eq!(
            TakeState::Candidate.transition(TakeTransition::Shortlist),
            Ok(TakeState::Shortlisted)
        );
        assert_eq!(
            TakeState::Shortlisted.transition(TakeTransition::Select),
            Ok(TakeState::Selected)
        );
        assert_eq!(
            TakeState::Selected.transition(TakeTransition::Approve),
            Ok(TakeState::Approved)
        );
        for terminal in [
            TakeState::Approved,
            TakeState::Rejected,
            TakeState::Superseded,
        ] {
            for transition in [
                TakeTransition::Shortlist,
                TakeTransition::Select,
                TakeTransition::Approve,
                TakeTransition::Reject,
                TakeTransition::Supersede,
            ] {
                assert_eq!(
                    terminal.transition(transition),
                    Err(CreativeError::InvalidTransition)
                );
            }
        }
    }
}
