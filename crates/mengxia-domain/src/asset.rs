use std::collections::BTreeSet;
use std::fmt;

use mengxia_types::{Id, RevisionNo, Sha256Digest, Timestamp};

const TOKEN_MAX_BYTES: usize = 64;
const LOGICAL_NAME_MAX_BYTES: usize = 255;
const MEDIA_TYPE_MAX_BYTES: usize = 255;
const MAX_PARENTS: usize = 64;

/// Marker for an Asset identity.
///
/// ```compile_fail
/// use mengxia_domain::{Asset, Location};
/// use mengxia_types::Id;
/// fn needs_asset(_: Id<Asset>) {}
/// needs_asset(Id::<Location>::try_new().unwrap());
/// ```
pub enum Asset {}
/// Marker for an AssetRevision identity.
///
/// ```compile_fail
/// use mengxia_domain::{Asset, AssetRevision};
/// use mengxia_types::Id;
/// fn needs_revision(_: Id<AssetRevision>) {}
/// needs_revision(Id::<Asset>::try_new().unwrap());
/// ```
pub enum AssetRevision {}
/// Marker for a Representation identity.
///
/// ```compile_fail
/// use mengxia_domain::{Representation, Resource};
/// use mengxia_types::Id;
/// fn needs_representation(_: Id<Representation>) {}
/// needs_representation(Id::<Resource>::try_new().unwrap());
/// ```
pub enum Representation {}
/// Marker for a Resource identity.
///
/// ```compile_fail
/// use mengxia_domain::{Representation, Resource};
/// use mengxia_types::Id;
/// fn needs_resource(_: Id<Resource>) {}
/// needs_resource(Id::<Representation>::try_new().unwrap());
/// ```
pub enum Resource {}
/// Marker for a Location identity.
///
/// ```compile_fail
/// use mengxia_domain::{AssetRevision, Location};
/// use mengxia_types::Id;
/// fn needs_location(_: Id<Location>) {}
/// needs_location(Id::<AssetRevision>::try_new().unwrap());
/// ```
pub enum Location {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetLifecycle {
    Active,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionCustody {
    Managed,
    Unmanaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobLifecycle {
    Available,
    GcPending,
    Purged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocationLifecycle {
    Available,
    Corrupt,
    Missing,
    Removed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocationCustody {
    Managed,
    Unmanaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocationDurability {
    Durable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvenanceVerification {
    Unknown,
    Verified,
    Conflicted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AssetError {
    InvalidValue,
    InvalidGraph,
    InvalidTransition,
    Conflict,
    RevisionExhausted,
}

impl fmt::Display for AssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidValue => "asset value validation failed",
            Self::InvalidGraph => "asset graph validation failed",
            Self::InvalidTransition => "asset transition is invalid",
            Self::Conflict => "asset revision conflict",
            Self::RevisionExhausted => "asset revision is exhausted",
        })
    }
}

impl std::error::Error for AssetError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Token(String);

impl Token {
    fn new(value: impl Into<String>) -> Result<Self, AssetError> {
        let value = value.into();
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > TOKEN_MAX_BYTES
            || !bytes[0].is_ascii_lowercase()
            || !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(AssetError::InvalidValue);
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! domain_token {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Token);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AssetError> {
                Token::new(value).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
    };
}

domain_token!(AssetKind);
domain_token!(ContentKind);
domain_token!(RepresentationPurpose);
domain_token!(ResourceKind);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalName(String);

impl LogicalName {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetError> {
        let value = value.into();
        if value.len() > LOGICAL_NAME_MAX_BYTES || value.chars().any(char::is_control) {
            return Err(AssetError::InvalidValue);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaType(String);

impl MediaType {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetError> {
        let value = value.into();
        if !(3..=MEDIA_TYPE_MAX_BYTES).contains(&value.len())
            || !value.is_ascii()
            || value.bytes().filter(|byte| *byte == b'/').count() != 1
            || value.starts_with('/')
            || value.ends_with('/')
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(
                        byte,
                        b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-' | b'/'
                    )
            })
        {
            return Err(AssetError::InvalidValue);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct RegisterManagedAssetValues {
    pub asset_id: Id<Asset>,
    pub asset_kind: AssetKind,
    pub asset_revision_id: Id<AssetRevision>,
    pub content_kind: ContentKind,
    pub representation_id: Id<Representation>,
    pub representation_purpose: RepresentationPurpose,
    pub resource_id: Id<Resource>,
    pub resource_kind: ResourceKind,
    pub logical_name: LogicalName,
    pub media_type: Option<MediaType>,
    pub blob_digest: Sha256Digest,
    pub created_at: Timestamp,
}

pub struct AssetGraph {
    asset: AssetRecord,
    asset_revision_id: Id<AssetRevision>,
    content_kind: ContentKind,
    representation_id: Id<Representation>,
    representation_purpose: RepresentationPurpose,
    resource_id: Id<Resource>,
    resource_kind: ResourceKind,
    logical_name: LogicalName,
    media_type: Option<MediaType>,
    blob_digest: Sha256Digest,
}

impl AssetGraph {
    pub fn register_managed(values: RegisterManagedAssetValues) -> Result<Self, AssetError> {
        Ok(Self {
            asset: AssetRecord {
                id: values.asset_id,
                kind: values.asset_kind,
                lifecycle: AssetLifecycle::Active,
                revision: RevisionNo::new(1),
                created_at: values.created_at,
            },
            asset_revision_id: values.asset_revision_id,
            content_kind: values.content_kind,
            representation_id: values.representation_id,
            representation_purpose: values.representation_purpose,
            resource_id: values.resource_id,
            resource_kind: values.resource_kind,
            logical_name: values.logical_name,
            media_type: values.media_type,
            blob_digest: values.blob_digest,
        })
    }

    #[must_use]
    pub const fn asset(&self) -> &AssetRecord {
        &self.asset
    }

    #[must_use]
    pub const fn asset_revision_id(&self) -> Id<AssetRevision> {
        self.asset_revision_id
    }

    #[must_use]
    pub const fn content_kind(&self) -> &ContentKind {
        &self.content_kind
    }

    #[must_use]
    pub const fn representation_id(&self) -> Id<Representation> {
        self.representation_id
    }

    #[must_use]
    pub const fn representation_purpose(&self) -> &RepresentationPurpose {
        &self.representation_purpose
    }

    #[must_use]
    pub const fn resource_id(&self) -> Id<Resource> {
        self.resource_id
    }

    #[must_use]
    pub const fn resource_kind(&self) -> &ResourceKind {
        &self.resource_kind
    }

    #[must_use]
    pub const fn logical_name(&self) -> &LogicalName {
        &self.logical_name
    }

    #[must_use]
    pub const fn media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }

    #[must_use]
    pub const fn blob_digest(&self) -> Sha256Digest {
        self.blob_digest
    }
}

pub struct AssetRecord {
    id: Id<Asset>,
    kind: AssetKind,
    lifecycle: AssetLifecycle,
    revision: RevisionNo,
    created_at: Timestamp,
}

impl AssetRecord {
    #[must_use]
    pub const fn id(&self) -> Id<Asset> {
        self.id
    }

    #[must_use]
    pub const fn kind(&self) -> &AssetKind {
        &self.kind
    }

    #[must_use]
    pub const fn lifecycle(&self) -> AssetLifecycle {
        self.lifecycle
    }

    #[must_use]
    pub const fn revision(&self) -> RevisionNo {
        self.revision
    }

    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    pub fn create_revision(
        &self,
        values: CreateAssetRevisionValues,
    ) -> Result<NewAssetRevision, AssetError> {
        if self.lifecycle != AssetLifecycle::Active || values.expected_revision != self.revision {
            return Err(AssetError::Conflict);
        }
        if values.parent_revision_ids.is_empty()
            || values.parent_revision_ids.len() > MAX_PARENTS
            || values.representations.is_empty()
            || values.representations.len() > 64
            || values.parent_revision_ids.contains(&values.revision_id)
            || values
                .parent_revision_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != values.parent_revision_ids.len()
        {
            return Err(AssetError::InvalidGraph);
        }
        let representation_ids = values
            .representations
            .iter()
            .map(|item| item.id)
            .collect::<BTreeSet<_>>();
        if representation_ids.len() != values.representations.len() {
            return Err(AssetError::InvalidGraph);
        }
        let resource_count = values
            .representations
            .iter()
            .map(|representation| representation.resources.len())
            .sum::<usize>();
        let resource_ids = values
            .representations
            .iter()
            .flat_map(|representation| representation.resources.iter().map(|resource| resource.id))
            .collect::<BTreeSet<_>>();
        if resource_ids.len() != resource_count {
            return Err(AssetError::InvalidGraph);
        }
        let resulting_revision = self
            .revision
            .checked_next()
            .map_err(|_| AssetError::RevisionExhausted)?;
        Ok(NewAssetRevision {
            asset_id: self.id,
            revision_id: values.revision_id,
            parent_revision_ids: values.parent_revision_ids,
            content_kind: values.content_kind,
            representations: values.representations,
            resulting_revision,
            created_at: values.created_at,
        })
    }
}

pub struct CreateAssetRevisionValues {
    pub expected_revision: RevisionNo,
    pub revision_id: Id<AssetRevision>,
    pub parent_revision_ids: Vec<Id<AssetRevision>>,
    pub content_kind: ContentKind,
    pub representations: Vec<RevisionRepresentation>,
    pub created_at: Timestamp,
}

pub struct RevisionRepresentation {
    id: Id<Representation>,
    purpose: RepresentationPurpose,
    resources: Vec<RevisionResource>,
}

impl RevisionRepresentation {
    pub fn new(
        id: Id<Representation>,
        purpose: RepresentationPurpose,
        resources: Vec<RevisionResource>,
    ) -> Result<Self, AssetError> {
        if resources.is_empty() || resources.len() > 64 {
            return Err(AssetError::InvalidGraph);
        }
        let unique = resources
            .iter()
            .map(|item| item.id)
            .collect::<BTreeSet<_>>();
        if unique.len() != resources.len() {
            return Err(AssetError::InvalidGraph);
        }
        Ok(Self {
            id,
            purpose,
            resources,
        })
    }
    #[must_use]
    pub const fn id(&self) -> Id<Representation> {
        self.id
    }
    #[must_use]
    pub const fn purpose(&self) -> &RepresentationPurpose {
        &self.purpose
    }
    #[must_use]
    pub fn resources(&self) -> &[RevisionResource] {
        &self.resources
    }
}

pub struct RevisionResource {
    id: Id<Resource>,
    kind: ResourceKind,
    members: Vec<RevisionMember>,
}

impl RevisionResource {
    pub fn new(
        id: Id<Resource>,
        kind: ResourceKind,
        members: Vec<RevisionMember>,
    ) -> Result<Self, AssetError> {
        if members.is_empty() || members.len() > 4096 {
            return Err(AssetError::InvalidGraph);
        }
        Ok(Self { id, kind, members })
    }
    #[must_use]
    pub const fn id(&self) -> Id<Resource> {
        self.id
    }
    #[must_use]
    pub const fn kind(&self) -> &ResourceKind {
        &self.kind
    }
    #[must_use]
    pub fn members(&self) -> &[RevisionMember] {
        &self.members
    }
}

pub struct RevisionMember {
    logical_name: LogicalName,
    blob_digest: Sha256Digest,
}

impl RevisionMember {
    #[must_use]
    pub const fn new(logical_name: LogicalName, blob_digest: Sha256Digest) -> Self {
        Self {
            logical_name,
            blob_digest,
        }
    }
    #[must_use]
    pub const fn logical_name(&self) -> &LogicalName {
        &self.logical_name
    }
    #[must_use]
    pub const fn blob_digest(&self) -> Sha256Digest {
        self.blob_digest
    }
}

pub struct NewAssetRevision {
    asset_id: Id<Asset>,
    revision_id: Id<AssetRevision>,
    parent_revision_ids: Vec<Id<AssetRevision>>,
    content_kind: ContentKind,
    representations: Vec<RevisionRepresentation>,
    resulting_revision: RevisionNo,
    created_at: Timestamp,
}

impl NewAssetRevision {
    #[must_use]
    pub const fn asset_id(&self) -> Id<Asset> {
        self.asset_id
    }
    #[must_use]
    pub const fn revision_id(&self) -> Id<AssetRevision> {
        self.revision_id
    }
    #[must_use]
    pub fn parent_revision_ids(&self) -> &[Id<AssetRevision>] {
        &self.parent_revision_ids
    }
    #[must_use]
    pub const fn content_kind(&self) -> &ContentKind {
        &self.content_kind
    }
    #[must_use]
    pub fn representations(&self) -> &[RevisionRepresentation] {
        &self.representations
    }
    #[must_use]
    pub const fn resulting_revision(&self) -> RevisionNo {
        self.resulting_revision
    }
    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }
}

pub struct RecordManagedLocationValues {
    pub digest: Sha256Digest,
    pub expected_revision: RevisionNo,
    pub location_id: Id<Location>,
    pub verified_at: Timestamp,
}

pub struct BlobRecord {
    digest: Sha256Digest,
    lifecycle: BlobLifecycle,
    revision: RevisionNo,
}

impl BlobRecord {
    #[must_use]
    pub const fn available(digest: Sha256Digest, revision: RevisionNo) -> Self {
        Self {
            digest,
            lifecycle: BlobLifecycle::Available,
            revision,
        }
    }

    pub fn record_location(
        &self,
        values: RecordManagedLocationValues,
    ) -> Result<LocationChange, AssetError> {
        if self.lifecycle != BlobLifecycle::Available || values.digest != self.digest {
            return Err(AssetError::InvalidTransition);
        }
        if values.expected_revision != self.revision {
            return Err(AssetError::Conflict);
        }
        let resulting_revision = self
            .revision
            .checked_next()
            .map_err(|_| AssetError::RevisionExhausted)?;
        Ok(LocationChange {
            digest: self.digest,
            location_id: values.location_id,
            resulting_revision,
            verified_at: values.verified_at,
        })
    }
}

pub struct LocationChange {
    digest: Sha256Digest,
    location_id: Id<Location>,
    resulting_revision: RevisionNo,
    verified_at: Timestamp,
}

impl LocationChange {
    #[must_use]
    pub const fn digest(&self) -> Sha256Digest {
        self.digest
    }
    #[must_use]
    pub const fn location_id(&self) -> Id<Location> {
        self.location_id
    }
    #[must_use]
    pub const fn resulting_revision(&self) -> RevisionNo {
        self.resulting_revision
    }
    #[must_use]
    pub const fn verified_at(&self) -> Timestamp {
        self.verified_at
    }
}

#[cfg(test)]
mod tests {
    use mengxia_types::{Id, RevisionNo, Sha256Digest, Timestamp};

    use super::{
        Asset, AssetError, AssetKind, AssetLifecycle, AssetRecord, AssetRevision, ContentKind,
        Location, LogicalName, MediaType, Representation, RepresentationPurpose, Resource,
        ResourceKind, RevisionMember, RevisionRepresentation, RevisionResource,
    };

    fn timestamp() -> Timestamp {
        Timestamp::from_unix_seconds_nanos(1_700_000_000, 123).unwrap()
    }

    fn asset_record(revision: u64) -> AssetRecord {
        AssetRecord {
            id: Id::<Asset>::try_new().unwrap(),
            kind: AssetKind::new("image").unwrap(),
            lifecycle: AssetLifecycle::Active,
            revision: RevisionNo::new(revision),
            created_at: timestamp(),
        }
    }

    fn representation(digest: Sha256Digest) -> RevisionRepresentation {
        RevisionRepresentation::new(
            Id::<Representation>::try_new().unwrap(),
            RepresentationPurpose::new("original").unwrap(),
            vec![
                RevisionResource::new(
                    Id::<Resource>::try_new().unwrap(),
                    ResourceKind::new("file").unwrap(),
                    vec![RevisionMember::new(
                        LogicalName::new("asset.bin").unwrap(),
                        digest,
                    )],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn bounded_tokens_names_and_media_types_reject_ambiguous_values() {
        assert!(AssetKind::new("a".repeat(63)).is_ok());
        assert!(AssetKind::new("a".repeat(64)).is_ok());
        assert_eq!(
            AssetKind::new("a".repeat(65)).unwrap_err(),
            AssetError::InvalidValue
        );
        assert!(AssetKind::new("Uppercase").is_err());
        assert!(LogicalName::new("x".repeat(254)).is_ok());
        assert!(LogicalName::new("x".repeat(255)).is_ok());
        assert!(LogicalName::new("x".repeat(256)).is_err());
        assert!(LogicalName::new("bad\nname").is_err());
        assert!(MediaType::new("image/png").is_ok());
        assert!(MediaType::new("image/png/extra").is_err());
        assert!(MediaType::new("Image/png").is_err());
        assert!(MediaType::new(format!("a/{}", "b".repeat(252))).is_ok());
        assert!(MediaType::new(format!("a/{}", "b".repeat(253))).is_ok());
        assert!(MediaType::new(format!("a/{}", "b".repeat(254))).is_err());
    }

    #[test]
    fn every_graph_collection_enforces_cap_minus_one_cap_and_cap_plus_one() {
        let digest = Sha256Digest::from_bytes([0x42; 32]);
        for (count, accepted) in [(4095, true), (4096, true), (4097, false)] {
            let members = (0..count)
                .map(|ordinal| {
                    RevisionMember::new(
                        LogicalName::new(format!("member-{ordinal}")).unwrap(),
                        digest,
                    )
                })
                .collect();
            assert_eq!(
                RevisionResource::new(
                    Id::<Resource>::try_new().unwrap(),
                    ResourceKind::new("file").unwrap(),
                    members,
                )
                .is_ok(),
                accepted,
                "member count {count}"
            );
        }

        for (count, accepted) in [(63, true), (64, true), (65, false)] {
            let resources = (0..count)
                .map(|ordinal| {
                    RevisionResource::new(
                        Id::<Resource>::try_new().unwrap(),
                        ResourceKind::new("file").unwrap(),
                        vec![RevisionMember::new(
                            LogicalName::new(format!("resource-{ordinal}")).unwrap(),
                            digest,
                        )],
                    )
                    .unwrap()
                })
                .collect();
            assert_eq!(
                RevisionRepresentation::new(
                    Id::<Representation>::try_new().unwrap(),
                    RepresentationPurpose::new("original").unwrap(),
                    resources,
                )
                .is_ok(),
                accepted,
                "resource count {count}"
            );
        }

        for (count, accepted) in [(63, true), (64, true), (65, false)] {
            let asset = asset_record(1);
            let parents = (0..count)
                .map(|_| Id::<AssetRevision>::try_new().unwrap())
                .collect();
            let result = asset.create_revision(super::CreateAssetRevisionValues {
                expected_revision: RevisionNo::new(1),
                revision_id: Id::try_new().unwrap(),
                parent_revision_ids: parents,
                content_kind: ContentKind::new("binary").unwrap(),
                representations: vec![representation(digest)],
                created_at: timestamp(),
            });
            assert_eq!(result.is_ok(), accepted, "parent count {count}");
        }

        for (count, accepted) in [(63, true), (64, true), (65, false)] {
            let asset = asset_record(1);
            let representations = (0..count).map(|_| representation(digest)).collect();
            let result = asset.create_revision(super::CreateAssetRevisionValues {
                expected_revision: RevisionNo::new(1),
                revision_id: Id::try_new().unwrap(),
                parent_revision_ids: vec![Id::try_new().unwrap()],
                content_kind: ContentKind::new("binary").unwrap(),
                representations,
                created_at: timestamp(),
            });
            assert_eq!(result.is_ok(), accepted, "representation count {count}");
        }
    }

    #[test]
    fn creative_revision_enforces_expected_revision_parent_and_collection_caps() {
        let asset = asset_record(1);
        let first_parent = Id::<AssetRevision>::try_new().unwrap();
        let new_revision = Id::<AssetRevision>::try_new().unwrap();
        let valid = asset
            .create_revision(super::CreateAssetRevisionValues {
                expected_revision: RevisionNo::new(1),
                revision_id: new_revision,
                parent_revision_ids: vec![first_parent],
                content_kind: ContentKind::new("binary").unwrap(),
                representations: vec![representation(Sha256Digest::from_bytes([7; 32]))],
                created_at: timestamp(),
            })
            .unwrap();
        assert_eq!(valid.resulting_revision(), RevisionNo::new(2));

        let duplicate_parent = asset.create_revision(super::CreateAssetRevisionValues {
            expected_revision: RevisionNo::new(1),
            revision_id: Id::try_new().unwrap(),
            parent_revision_ids: vec![first_parent, first_parent],
            content_kind: ContentKind::new("binary").unwrap(),
            representations: vec![representation(Sha256Digest::from_bytes([8; 32]))],
            created_at: timestamp(),
        });
        assert!(matches!(duplicate_parent, Err(AssetError::InvalidGraph)));

        let wrong_expected = asset.create_revision(super::CreateAssetRevisionValues {
            expected_revision: RevisionNo::new(2),
            revision_id: Id::try_new().unwrap(),
            parent_revision_ids: vec![first_parent],
            content_kind: ContentKind::new("binary").unwrap(),
            representations: vec![representation(Sha256Digest::from_bytes([9; 32]))],
            created_at: timestamp(),
        });
        assert!(matches!(wrong_expected, Err(AssetError::Conflict)));
    }

    #[test]
    fn revision_rejects_resource_identity_reused_across_representations() {
        let shared_resource_id = Id::<Resource>::try_new().unwrap();
        let digest = Sha256Digest::from_bytes([0x5a; 32]);
        let representations = ["original", "thumbnail"]
            .into_iter()
            .map(|purpose| {
                RevisionRepresentation::new(
                    Id::<Representation>::try_new().unwrap(),
                    RepresentationPurpose::new(purpose).unwrap(),
                    vec![
                        RevisionResource::new(
                            shared_resource_id,
                            ResourceKind::new("file").unwrap(),
                            vec![RevisionMember::new(
                                LogicalName::new(format!("{purpose}.bin")).unwrap(),
                                digest,
                            )],
                        )
                        .unwrap(),
                    ],
                )
                .unwrap()
            })
            .collect();

        let result = asset_record(1).create_revision(super::CreateAssetRevisionValues {
            expected_revision: RevisionNo::new(1),
            revision_id: Id::try_new().unwrap(),
            parent_revision_ids: vec![Id::try_new().unwrap()],
            content_kind: ContentKind::new("binary").unwrap(),
            representations,
            created_at: timestamp(),
        });

        assert!(matches!(result, Err(AssetError::InvalidGraph)));
    }

    #[test]
    fn blob_location_transition_keeps_blob_and_location_identity_distinct() {
        let digest = Sha256Digest::from_bytes([3; 32]);
        let location = Id::<Location>::try_new().unwrap();
        let change = super::BlobRecord::available(digest, RevisionNo::new(4))
            .record_location(super::RecordManagedLocationValues {
                digest,
                expected_revision: RevisionNo::new(4),
                location_id: location,
                verified_at: timestamp(),
            })
            .unwrap();
        assert_eq!(change.digest(), digest);
        assert_eq!(change.location_id(), location);
        assert_eq!(change.resulting_revision(), RevisionNo::new(5));
    }
}
