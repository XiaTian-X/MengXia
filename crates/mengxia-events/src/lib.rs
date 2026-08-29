//! Domain and security event contracts for MengXia.

#![forbid(unsafe_code)]

use mengxia_types::{Id, RevisionNo, Sha256Digest, Timestamp};

/// Type-level identity marker for committed domain events.
///
/// ```compile_fail
/// use mengxia_events::{DomainEvent, ProvenanceEvent};
/// use mengxia_types::Id;
/// let domain = Id::<DomainEvent>::try_new().unwrap();
/// let _: Id<ProvenanceEvent> = domain;
/// ```
pub enum DomainEvent {}

/// Type-level identity marker for provenance events.
pub enum ProvenanceEvent {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregateRef {
    Asset([u8; 16]),
    AssetRevision([u8; 16]),
    Blob(Sha256Digest),
    Location([u8; 16]),
}

pub struct DomainEventRecord {
    event_id: Id<DomainEvent>,
    event_type: &'static str,
    aggregate: AggregateRef,
    aggregate_revision: Option<RevisionNo>,
    occurred_at: Timestamp,
}

impl DomainEventRecord {
    #[must_use]
    pub const fn asset_registered(
        event_id: Id<DomainEvent>,
        asset_id: [u8; 16],
        revision: RevisionNo,
        occurred_at: Timestamp,
    ) -> Self {
        Self {
            event_id,
            event_type: "asset.registered.v1",
            aggregate: AggregateRef::Asset(asset_id),
            aggregate_revision: Some(revision),
            occurred_at,
        }
    }

    #[must_use]
    pub const fn asset_revision_created(
        event_id: Id<DomainEvent>,
        revision_id: [u8; 16],
        revision: RevisionNo,
        occurred_at: Timestamp,
    ) -> Self {
        Self {
            event_id,
            event_type: "asset.revision.created.v1",
            aggregate: AggregateRef::AssetRevision(revision_id),
            aggregate_revision: Some(revision),
            occurred_at,
        }
    }

    #[must_use]
    pub const fn blob_location_recorded(
        event_id: Id<DomainEvent>,
        digest: Sha256Digest,
        revision: RevisionNo,
        occurred_at: Timestamp,
    ) -> Self {
        Self {
            event_id,
            event_type: "blob.location.recorded.v1",
            aggregate: AggregateRef::Blob(digest),
            aggregate_revision: Some(revision),
            occurred_at,
        }
    }

    #[must_use]
    pub const fn event_id(&self) -> Id<DomainEvent> {
        self.event_id
    }
    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        self.event_type
    }
    #[must_use]
    pub const fn aggregate(&self) -> AggregateRef {
        self.aggregate
    }
    #[must_use]
    pub const fn aggregate_revision(&self) -> Option<RevisionNo> {
        self.aggregate_revision
    }
    #[must_use]
    pub const fn occurred_at(&self) -> Timestamp {
        self.occurred_at
    }
    #[must_use]
    pub const fn schema_version(&self) -> i64 {
        1
    }
}

pub struct ProvenanceEventRecord {
    event_id: Id<ProvenanceEvent>,
    event_type: &'static str,
    asset_revision_id: [u8; 16],
    blob_digest: Option<Sha256Digest>,
    occurred_at: Timestamp,
}

impl ProvenanceEventRecord {
    #[must_use]
    pub const fn asset_ingested_copy(
        event_id: Id<ProvenanceEvent>,
        asset_revision_id: [u8; 16],
        digest: Sha256Digest,
        occurred_at: Timestamp,
    ) -> Self {
        Self {
            event_id,
            event_type: "asset.ingested.copy.v1",
            asset_revision_id,
            blob_digest: Some(digest),
            occurred_at,
        }
    }

    #[must_use]
    pub const fn asset_revision_derived(
        event_id: Id<ProvenanceEvent>,
        asset_revision_id: [u8; 16],
        occurred_at: Timestamp,
    ) -> Self {
        Self {
            event_id,
            event_type: "asset.revision.derived.v1",
            asset_revision_id,
            blob_digest: None,
            occurred_at,
        }
    }

    #[must_use]
    pub const fn event_id(&self) -> Id<ProvenanceEvent> {
        self.event_id
    }
    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        self.event_type
    }
    #[must_use]
    pub const fn asset_revision_id_bytes(&self) -> [u8; 16] {
        self.asset_revision_id
    }
    #[must_use]
    pub const fn blob_digest(&self) -> Option<Sha256Digest> {
        self.blob_digest
    }
    #[must_use]
    pub const fn occurred_at(&self) -> Timestamp {
        self.occurred_at
    }
    #[must_use]
    pub const fn schema_version(&self) -> i64 {
        1
    }
}

#[cfg(test)]
mod tests {
    use mengxia_types::{Id, RevisionNo, Sha256Digest, Timestamp};

    use super::{
        AggregateRef, DomainEvent, DomainEventRecord, ProvenanceEvent, ProvenanceEventRecord,
    };

    #[test]
    fn operation_owned_event_shapes_cannot_accept_caller_event_metadata() {
        let at = Timestamp::from_unix_seconds_nanos(1_700_000_000, 9).unwrap();
        let domain_id = Id::<DomainEvent>::try_new().unwrap();
        let asset_id = Id::<()>::try_new().unwrap().to_bytes();
        let event =
            DomainEventRecord::asset_registered(domain_id, asset_id, RevisionNo::new(1), at);
        assert_eq!(event.event_type(), "asset.registered.v1");
        assert_eq!(event.aggregate(), AggregateRef::Asset(asset_id));
        assert_eq!(event.aggregate_revision(), Some(RevisionNo::new(1)));
        assert_eq!(event.schema_version(), 1);

        let digest = Sha256Digest::from_bytes([4; 32]);
        let provenance = ProvenanceEventRecord::asset_ingested_copy(
            Id::<ProvenanceEvent>::try_new().unwrap(),
            asset_id,
            digest,
            at,
        );
        assert_eq!(provenance.event_type(), "asset.ingested.copy.v1");
        assert_eq!(provenance.blob_digest(), Some(digest));
        assert_eq!(provenance.schema_version(), 1);
    }
}
