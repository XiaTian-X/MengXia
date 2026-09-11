use mengxia_types::{Id, Sha256Digest, Timestamp};
use sha2::{Digest, Sha256};

use super::StoreError;
use super::migration::LibraryIdentity;

pub(crate) const MIGRATION_INTENT_BYTES: usize = 512;
const MAGIC: &[u8; 16] = b"MENGXIA_MIG2_V1\0";
const VERSION: u16 = 1;
const MIGRATION_SEQUENCE: u16 = 2;
const MIGRATION_NAME: &[u8; 18] = b"0002_projects_work";

pub(crate) enum MigrationAttemptIdentity {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MigrationIntent {
    attempt_id: Id<MigrationAttemptIdentity>,
    library_id: Id<LibraryIdentity>,
    root_device: u64,
    root_inode: u64,
    source_device: u64,
    source_inode: u64,
    source_length: u64,
    source_sha256: Sha256Digest,
    migration_prefix_sha256: Sha256Digest,
    created_at: Timestamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MigrationIntentInput {
    pub(crate) attempt_id: Id<MigrationAttemptIdentity>,
    pub(crate) library_id: Id<LibraryIdentity>,
    pub(crate) root_device: u64,
    pub(crate) root_inode: u64,
    pub(crate) source_device: u64,
    pub(crate) source_inode: u64,
    pub(crate) source_length: u64,
    pub(crate) source_sha256: Sha256Digest,
    pub(crate) migration_prefix_sha256: Sha256Digest,
    pub(crate) created_at: Timestamp,
}

impl MigrationIntent {
    pub(crate) const fn new(input: MigrationIntentInput) -> Self {
        Self {
            attempt_id: input.attempt_id,
            library_id: input.library_id,
            root_device: input.root_device,
            root_inode: input.root_inode,
            source_device: input.source_device,
            source_inode: input.source_inode,
            source_length: input.source_length,
            source_sha256: input.source_sha256,
            migration_prefix_sha256: input.migration_prefix_sha256,
            created_at: input.created_at,
        }
    }

    pub(crate) fn encode(self) -> [u8; MIGRATION_INTENT_BYTES] {
        let mut record = [0_u8; MIGRATION_INTENT_BYTES];
        record[0..16].copy_from_slice(MAGIC);
        record[16..18].copy_from_slice(&VERSION.to_be_bytes());
        record[18..20].copy_from_slice(&(MIGRATION_INTENT_BYTES as u16).to_be_bytes());
        record[24..40].copy_from_slice(&self.attempt_id.to_bytes());
        record[40..56].copy_from_slice(&self.library_id.to_bytes());
        record[56..64].copy_from_slice(&self.root_device.to_be_bytes());
        record[64..72].copy_from_slice(&self.root_inode.to_be_bytes());
        record[72..80].copy_from_slice(&self.source_device.to_be_bytes());
        record[80..88].copy_from_slice(&self.source_inode.to_be_bytes());
        record[88..96].copy_from_slice(&self.source_length.to_be_bytes());
        record[96..128].copy_from_slice(&self.source_sha256.to_bytes());
        record[128..160].copy_from_slice(&self.migration_prefix_sha256.to_bytes());
        record[160..192].copy_from_slice(&super::migration::CREATIVE_MIGRATION_SHA256);
        record[192..200].copy_from_slice(&self.created_at.unix_seconds().to_be_bytes());
        record[200..204].copy_from_slice(&self.created_at.subsec_nanoseconds().to_be_bytes());
        record[204..206].copy_from_slice(&MIGRATION_SEQUENCE.to_be_bytes());
        record[206..208].copy_from_slice(&(MIGRATION_NAME.len() as u16).to_be_bytes());
        record[208..226].copy_from_slice(MIGRATION_NAME);
        let checksum: [u8; 32] = Sha256::digest(&record[..480]).into();
        record[480..512].copy_from_slice(&checksum);
        record
    }

    pub(crate) fn decode(record: &[u8; MIGRATION_INTENT_BYTES]) -> Result<Self, StoreError> {
        let checksum: [u8; 32] = Sha256::digest(&record[..480]).into();
        if &record[0..16] != MAGIC
            || u16::from_be_bytes(record[16..18].try_into().expect("fixed slice")) != VERSION
            || u16::from_be_bytes(record[18..20].try_into().expect("fixed slice"))
                != MIGRATION_INTENT_BYTES as u16
            || record[20..24] != [0; 4]
            || record[160..192] != super::migration::CREATIVE_MIGRATION_SHA256
            || u16::from_be_bytes(record[204..206].try_into().expect("fixed slice"))
                != MIGRATION_SEQUENCE
            || u16::from_be_bytes(record[206..208].try_into().expect("fixed slice"))
                != MIGRATION_NAME.len() as u16
            || &record[208..226] != MIGRATION_NAME
            || record[226..480].iter().any(|byte| *byte != 0)
            || record[480..512] != checksum
        {
            return Err(StoreError::Corruption);
        }

        let attempt_id = Id::from_bytes(record[24..40].try_into().expect("fixed slice"))
            .map_err(|_| StoreError::Corruption)?;
        let library_id = Id::from_bytes(record[40..56].try_into().expect("fixed slice"))
            .map_err(|_| StoreError::Corruption)?;
        let created_at = Timestamp::from_unix_seconds_nanos(
            i64::from_be_bytes(record[192..200].try_into().expect("fixed slice")),
            u32::from_be_bytes(record[200..204].try_into().expect("fixed slice")),
        )
        .map_err(|_| StoreError::Corruption)?;

        Ok(Self {
            attempt_id,
            library_id,
            root_device: u64::from_be_bytes(record[56..64].try_into().expect("fixed slice")),
            root_inode: u64::from_be_bytes(record[64..72].try_into().expect("fixed slice")),
            source_device: u64::from_be_bytes(record[72..80].try_into().expect("fixed slice")),
            source_inode: u64::from_be_bytes(record[80..88].try_into().expect("fixed slice")),
            source_length: u64::from_be_bytes(record[88..96].try_into().expect("fixed slice")),
            source_sha256: Sha256Digest::from_bytes(
                record[96..128].try_into().expect("fixed slice"),
            ),
            migration_prefix_sha256: Sha256Digest::from_bytes(
                record[128..160].try_into().expect("fixed slice"),
            ),
            created_at,
        })
    }

    pub(crate) const fn library_id(self) -> Id<LibraryIdentity> {
        self.library_id
    }

    pub(crate) const fn root_identity(self) -> (u64, u64) {
        (self.root_device, self.root_inode)
    }

    pub(crate) const fn source_identity(self) -> (u64, u64, u64) {
        (self.source_device, self.source_inode, self.source_length)
    }

    pub(crate) const fn source_sha256(self) -> Sha256Digest {
        self.source_sha256
    }

    pub(crate) const fn migration_prefix_sha256(self) -> Sha256Digest {
        self.migration_prefix_sha256
    }
}

#[cfg(test)]
mod tests {
    use mengxia_types::{Id, Sha256Digest, Timestamp};

    use super::{MigrationAttemptIdentity, MigrationIntent, MigrationIntentInput};
    use crate::migration::LibraryIdentity;

    fn intent() -> MigrationIntent {
        MigrationIntent::new(MigrationIntentInput {
            attempt_id: Id::<MigrationAttemptIdentity>::from_bytes([
                0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
                0x77, 0x88,
            ])
            .unwrap(),
            library_id: Id::<LibraryIdentity>::from_bytes([
                0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x01, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
                0x77, 0x99,
            ])
            .unwrap(),
            root_device: 11,
            root_inode: 12,
            source_device: 13,
            source_inode: 14,
            source_length: 4096,
            source_sha256: Sha256Digest::from_bytes([0x55; 32]),
            migration_prefix_sha256: Sha256Digest::from_bytes([0xaa; 32]),
            created_at: Timestamp::from_unix_seconds_nanos(1_700_000_000, 123_456_789).unwrap(),
        })
    }

    #[test]
    fn exact_layout_round_trips_and_reserved_bytes_are_zero() {
        let intent = intent();
        let record = intent.encode();
        assert_eq!(record.len(), 512);
        assert_eq!(&record[0..16], b"MENGXIA_MIG2_V1\0");
        assert_eq!(&record[208..226], b"0002_projects_work");
        assert!(record[20..24].iter().all(|byte| *byte == 0));
        assert!(record[226..480].iter().all(|byte| *byte == 0));
        assert_eq!(MigrationIntent::decode(&record), Ok(intent));
    }

    #[test]
    fn every_field_region_and_checksum_fail_closed() {
        for offset in [
            0_usize, 16, 18, 20, 24, 40, 56, 64, 72, 80, 88, 96, 128, 160, 192, 200, 204, 206, 208,
            226, 479, 480, 511,
        ] {
            let mut record = intent().encode();
            record[offset] ^= 1;
            assert!(MigrationIntent::decode(&record).is_err(), "offset {offset}");
        }
    }
}
