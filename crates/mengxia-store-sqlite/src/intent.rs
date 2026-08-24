use mengxia_platform_fs::{BOOTSTRAP_INTENT_RECORD_LENGTH, OpenedLibraryAuthority};
use mengxia_types::{Id, Sha256Digest, Timestamp};
use sha2::{Digest, Sha256};

use super::StoreError;
use super::error::map_authority_error;
use super::migration::{LibraryIdentity, MIGRATION_NAME, MIGRATION_SEQUENCE, MIGRATION_SHA256};

pub(crate) const BOOTSTRAP_INTENT_LENGTH: usize = BOOTSTRAP_INTENT_RECORD_LENGTH;
const CHECKSUM_OFFSET: usize = 224;
const MAGIC: &[u8; 8] = b"MXBTINT1";
const VERSION: u16 = 1;
const HEADER_LENGTH: u16 = 76;
const CANONICAL_BASENAME: &[u8] = b"library.sqlite3";
const STAGING_BASENAME: &[u8] = b".library.sqlite3.bootstrap";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BootstrapIntent {
    root_device: u64,
    root_inode: u64,
    owner_uid: u32,
    library_id: Id<LibraryIdentity>,
    created_at: Timestamp,
    migration_digest: Sha256Digest,
}

impl BootstrapIntent {
    pub(crate) fn create_durable(
        authority: &OpenedLibraryAuthority,
        library_id: Id<LibraryIdentity>,
        created_at: Timestamp,
    ) -> Result<Self, StoreError> {
        let intent = Self::for_authority(authority, library_id, created_at);
        authority
            .create_durable_bootstrap_intent(&intent.encode())
            .map_err(map_authority_error)?;
        Ok(intent)
    }

    pub(crate) fn for_authority(
        authority: &OpenedLibraryAuthority,
        library_id: Id<LibraryIdentity>,
        created_at: Timestamp,
    ) -> Self {
        let (root_device, root_inode) = authority.root_identity();
        Self {
            root_device,
            root_inode,
            owner_uid: authority.owner_uid(),
            library_id,
            created_at,
            migration_digest: Sha256Digest::from_bytes(MIGRATION_SHA256),
        }
    }

    pub(crate) fn encode(self) -> [u8; BOOTSTRAP_INTENT_LENGTH] {
        let mut record = [0_u8; BOOTSTRAP_INTENT_LENGTH];
        record[0..8].copy_from_slice(MAGIC);
        record[8..10].copy_from_slice(&VERSION.to_be_bytes());
        record[10..12].copy_from_slice(&HEADER_LENGTH.to_be_bytes());
        record[12..16].copy_from_slice(&(BOOTSTRAP_INTENT_LENGTH as u32).to_be_bytes());
        record[16..24].copy_from_slice(&self.root_device.to_be_bytes());
        record[24..32].copy_from_slice(&self.root_inode.to_be_bytes());
        record[32..36].copy_from_slice(&self.owner_uid.to_be_bytes());
        record[40..56].copy_from_slice(&self.library_id.to_bytes());
        record[56..64].copy_from_slice(&self.created_at.unix_seconds().to_be_bytes());
        record[64..68].copy_from_slice(&self.created_at.subsec_nanoseconds().to_be_bytes());
        record[68..72].copy_from_slice(&(MIGRATION_SEQUENCE as u32).to_be_bytes());
        record[72] = MIGRATION_NAME.len() as u8;
        record[73] = CANONICAL_BASENAME.len() as u8;
        record[74] = STAGING_BASENAME.len() as u8;
        record[76..76 + MIGRATION_NAME.len()].copy_from_slice(MIGRATION_NAME.as_bytes());
        record[108..108 + CANONICAL_BASENAME.len()].copy_from_slice(CANONICAL_BASENAME);
        record[140..140 + STAGING_BASENAME.len()].copy_from_slice(STAGING_BASENAME);
        record[172..204].copy_from_slice(&self.migration_digest.to_bytes());
        let checksum = Sha256::digest(&record[..CHECKSUM_OFFSET]);
        record[CHECKSUM_OFFSET..].copy_from_slice(&checksum);
        record
    }

    pub(crate) fn decode(record: &[u8]) -> Result<Self, StoreError> {
        let record: &[u8; BOOTSTRAP_INTENT_LENGTH] =
            record.try_into().map_err(|_| StoreError::Configuration)?;
        let expected_checksum = Sha256::digest(&record[..CHECKSUM_OFFSET]);
        if record[CHECKSUM_OFFSET..] != expected_checksum[..]
            || &record[0..8] != MAGIC
            || read_u16(record, 8) != VERSION
            || read_u16(record, 10) != HEADER_LENGTH
            || read_u32(record, 12) != BOOTSTRAP_INTENT_LENGTH as u32
            || record[36..40] != [0; 4]
            || read_u32(record, 68) != MIGRATION_SEQUENCE as u32
            || usize::from(record[72]) != MIGRATION_NAME.len()
            || usize::from(record[73]) != CANONICAL_BASENAME.len()
            || usize::from(record[74]) != STAGING_BASENAME.len()
            || record[75] != 0
            || !slot_is_exact(record, 76, 32, MIGRATION_NAME.as_bytes())
            || !slot_is_exact(record, 108, 32, CANONICAL_BASENAME)
            || !slot_is_exact(record, 140, 32, STAGING_BASENAME)
            || record[204..CHECKSUM_OFFSET] != [0; 20]
        {
            return Err(StoreError::Configuration);
        }

        let library_id =
            Id::from_bytes(read_array(record, 40)).map_err(|_| StoreError::Configuration)?;
        let created_at =
            Timestamp::from_unix_seconds_nanos(read_i64(record, 56), read_u32(record, 64))
                .map_err(|_| StoreError::Configuration)?;

        Ok(Self {
            root_device: read_u64(record, 16),
            root_inode: read_u64(record, 24),
            owner_uid: read_u32(record, 32),
            library_id,
            created_at,
            migration_digest: Sha256Digest::from_bytes(read_array(record, 172)),
        })
    }

    pub(crate) fn verify_authority(
        self,
        authority: &OpenedLibraryAuthority,
    ) -> Result<(), StoreError> {
        let (root_device, root_inode) = authority.root_identity();
        if self.root_device == root_device
            && self.root_inode == root_inode
            && self.owner_uid == authority.owner_uid()
            && self.migration_digest.to_bytes() == MIGRATION_SHA256
        {
            Ok(())
        } else {
            Err(StoreError::Configuration)
        }
    }

    pub(crate) fn create_staging(
        self,
        authority: &OpenedLibraryAuthority,
    ) -> Result<(), StoreError> {
        self.verify_authority(authority)?;
        authority
            .refsync_intent_and_create_staging(&self.encode())
            .map_err(map_authority_error)
    }

    pub(crate) const fn library_id(self) -> Id<LibraryIdentity> {
        self.library_id
    }

    pub(crate) const fn created_at(self) -> Timestamp {
        self.created_at
    }
}

fn slot_is_exact(
    record: &[u8; BOOTSTRAP_INTENT_LENGTH],
    offset: usize,
    width: usize,
    value: &[u8],
) -> bool {
    value.is_ascii()
        && !value.contains(&0)
        && record[offset..offset + value.len()] == *value
        && record[offset + value.len()..offset + width]
            .iter()
            .all(|byte| *byte == 0)
}

fn read_array<const LENGTH: usize>(
    record: &[u8; BOOTSTRAP_INTENT_LENGTH],
    offset: usize,
) -> [u8; LENGTH] {
    record[offset..offset + LENGTH]
        .try_into()
        .expect("fixed intent offsets are in bounds")
}

fn read_u16(record: &[u8; BOOTSTRAP_INTENT_LENGTH], offset: usize) -> u16 {
    u16::from_be_bytes(read_array(record, offset))
}

fn read_u32(record: &[u8; BOOTSTRAP_INTENT_LENGTH], offset: usize) -> u32 {
    u32::from_be_bytes(read_array(record, offset))
}

fn read_u64(record: &[u8; BOOTSTRAP_INTENT_LENGTH], offset: usize) -> u64 {
    u64::from_be_bytes(read_array(record, offset))
}

fn read_i64(record: &[u8; BOOTSTRAP_INTENT_LENGTH], offset: usize) -> i64 {
    i64::from_be_bytes(read_array(record, offset))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::PathBuf;

    use mengxia_platform_fs::OpenedLibraryAuthority;
    use mengxia_types::{Id, Sha256Digest, Timestamp};
    use sha2::{Digest, Sha256};

    use super::{BOOTSTRAP_INTENT_LENGTH, BootstrapIntent, CHECKSUM_OFFSET};
    use crate::StoreError;
    use crate::migration::LibraryIdentity;

    const GOLDEN_HEX: &str = concat!(
        "4d584254494e54310001004c0000010001020304050607081112131415161718",
        "000001f50000000001890f1de00070008000000000000001000000006553f100",
        "075bcd1500000000140f1a00303030305f73746f72655f626f6f747374726170",
        "0000000000000000000000006c6962726172792e73716c697465330000000000",
        "0000000000000000000000002e6c6962726172792e73716c697465332e626f6f",
        "747374726170000000000000000102030405060708090a0b0c0d0e0f10111213",
        "1415161718191a1b1c1d1e1f0000000000000000000000000000000000000000",
        "61d3132622fa1ef1e69b1062be3b1a0eb4af990ce36153a041f7a4dce8a180f7",
    );

    #[test]
    fn exact_golden_vector_encodes_and_decodes() {
        let golden = golden_record();
        let intent = golden_intent();
        assert_eq!(intent.encode(), golden);
        assert_eq!(BootstrapIntent::decode(&golden), Ok(intent));
        assert_eq!(
            intent.library_id().to_bytes(),
            golden_library_id().to_bytes()
        );
        assert_eq!(intent.created_at(), golden_timestamp());
    }

    #[test]
    fn independent_sha256_proves_the_exact_coverage_boundary() {
        assert_eq!(
            reference_sha256(b""),
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55,
            ]
        );
        assert_eq!(
            reference_sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
        let golden = golden_record();
        let independent = reference_sha256(&golden[..CHECKSUM_OFFSET]);
        assert_eq!(independent, golden[CHECKSUM_OFFSET..]);
        assert_ne!(
            reference_sha256(&golden[..CHECKSUM_OFFSET - 1]),
            independent
        );
        assert_ne!(
            reference_sha256(&golden[..CHECKSUM_OFFSET + 1]),
            independent
        );
    }

    #[test]
    fn every_fixed_field_reserved_padding_and_checksum_boundary_fails_closed() {
        let golden = golden_record();
        assert_eq!(
            BootstrapIntent::decode(&golden[..BOOTSTRAP_INTENT_LENGTH - 1]),
            Err(StoreError::Configuration)
        );
        let mut trailing = golden.to_vec();
        trailing.push(0);
        assert_eq!(
            BootstrapIntent::decode(&trailing),
            Err(StoreError::Configuration)
        );
        for offset in 0..8 {
            assert_structural_mutation_fails(golden, offset);
        }
        for range in [
            8..10,
            10..12,
            12..16,
            36..40,
            68..72,
            72..76,
            76..96,
            96..108,
            108..123,
            123..140,
            140..166,
            166..172,
            204..224,
        ] {
            for offset in range {
                assert_structural_mutation_fails(golden, offset);
            }
        }

        for offset in CHECKSUM_OFFSET..BOOTSTRAP_INTENT_LENGTH {
            let mut corrupt = golden;
            corrupt[offset] ^= 1;
            assert_eq!(
                BootstrapIntent::decode(&corrupt),
                Err(StoreError::Configuration),
                "checksum byte {offset}"
            );
        }
    }

    #[test]
    fn typed_uuid_and_timestamp_failures_are_rejected_after_valid_rechecksum() {
        let golden = golden_record();

        let mut wrong_version = golden;
        wrong_version[46] = (wrong_version[46] & 0x0f) | 0x40;
        rechecksum(&mut wrong_version);
        assert_eq!(
            BootstrapIntent::decode(&wrong_version),
            Err(StoreError::Configuration)
        );

        let mut wrong_variant = golden;
        wrong_variant[48] &= 0x3f;
        rechecksum(&mut wrong_variant);
        assert_eq!(
            BootstrapIntent::decode(&wrong_variant),
            Err(StoreError::Configuration)
        );

        let mut seconds_out_of_range = golden;
        seconds_out_of_range[56..64].copy_from_slice(&i64::MAX.to_be_bytes());
        rechecksum(&mut seconds_out_of_range);
        assert_eq!(
            BootstrapIntent::decode(&seconds_out_of_range),
            Err(StoreError::Configuration)
        );

        let mut nanos_out_of_range = golden;
        nanos_out_of_range[64..68].copy_from_slice(&1_000_000_000_u32.to_be_bytes());
        rechecksum(&mut nanos_out_of_range);
        assert_eq!(
            BootstrapIntent::decode(&nanos_out_of_range),
            Err(StoreError::Configuration)
        );
    }

    #[test]
    fn variable_fields_use_big_endian_and_round_trip_without_native_layout() {
        let mut record = golden_record();
        record[16..24].copy_from_slice(&0x8877_6655_4433_2211_u64.to_be_bytes());
        record[24..32].copy_from_slice(&0x8070_6050_4030_2010_u64.to_be_bytes());
        record[32..36].copy_from_slice(&u32::MAX.to_be_bytes());
        record[55] ^= 1;
        record[56..64].copy_from_slice(&1_700_000_001_i64.to_be_bytes());
        record[64..68].copy_from_slice(&987_654_321_u32.to_be_bytes());
        record[172..204].fill(0xa5);
        rechecksum(&mut record);

        let decoded = BootstrapIntent::decode(&record).expect("valid variable fields");
        assert_eq!(decoded.root_device, 0x8877_6655_4433_2211);
        assert_eq!(decoded.root_inode, 0x8070_6050_4030_2010);
        assert_eq!(decoded.owner_uid, u32::MAX);
        assert_eq!(decoded.library_id().to_bytes(), record[40..56]);
        assert_eq!(decoded.created_at().unix_seconds(), 1_700_000_001);
        assert_eq!(decoded.created_at().subsec_nanoseconds(), 987_654_321);
        assert_eq!(decoded.migration_digest.to_bytes(), [0xa5; 32]);
    }

    #[test]
    fn real_held_root_authority_and_migration_identity_must_match() {
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("crate is inside workspace")
            .to_path_buf();
        let parent = repository.join(format!(
            "target/task-004-intent-authority-{}",
            std::process::id()
        ));
        if parent.exists() {
            fs::remove_dir_all(&parent).expect("remove stale intent fixture");
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&parent)
            .expect("create secure intent fixture parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure intent fixture parent");
        let authority = OpenedLibraryAuthority::acquire_bootstrap(&parent.join("Library"))
            .expect("acquire root and lock authority");
        let intent =
            BootstrapIntent::create_durable(&authority, golden_library_id(), golden_timestamp())
                .expect("create durable typed intent");
        let intent_path = parent.join("Library/.mengxia.bootstrap-intent");
        let persisted = fs::read(&intent_path).expect("read persisted typed intent");
        let decoded = BootstrapIntent::decode(&persisted).expect("decode persisted intent");
        decoded
            .verify_authority(&authority)
            .expect("matching held authority");
        assert_eq!(decoded, intent);
        assert_eq!(
            BootstrapIntent::create_durable(&authority, golden_library_id(), golden_timestamp()),
            Err(StoreError::Configuration)
        );
        assert_eq!(
            fs::read(&intent_path).expect("existing intent is preserved"),
            persisted
        );

        intent
            .create_staging(&authority)
            .expect("typed valid intent creates fixed empty staging");
        let staging = fs::metadata(parent.join("Library/.library.sqlite3.bootstrap"))
            .expect("created staging metadata");
        assert_eq!(staging.len(), 0);
        assert_eq!(staging.permissions().mode() & 0o777, 0o600);
        assert!(!parent.join("Library/library.sqlite3").exists());

        for mismatch in [
            BootstrapIntent {
                root_device: intent.root_device ^ 1,
                ..intent
            },
            BootstrapIntent {
                root_inode: intent.root_inode ^ 1,
                ..intent
            },
            BootstrapIntent {
                owner_uid: intent.owner_uid ^ 1,
                ..intent
            },
            BootstrapIntent {
                migration_digest: Sha256Digest::from_bytes([0xff; 32]),
                ..intent
            },
        ] {
            assert_eq!(
                mismatch.verify_authority(&authority),
                Err(StoreError::Configuration)
            );
        }
        drop(authority);
        fs::remove_dir_all(parent).expect("remove intent fixture");
    }

    fn assert_structural_mutation_fails(golden: [u8; BOOTSTRAP_INTENT_LENGTH], offset: usize) {
        let mut corrupt = golden;
        corrupt[offset] ^= 1;
        rechecksum(&mut corrupt);
        assert_eq!(
            BootstrapIntent::decode(&corrupt),
            Err(StoreError::Configuration),
            "structural byte {offset}"
        );
    }

    fn rechecksum(record: &mut [u8; BOOTSTRAP_INTENT_LENGTH]) {
        let checksum = Sha256::digest(&record[..CHECKSUM_OFFSET]);
        record[CHECKSUM_OFFSET..].copy_from_slice(&checksum);
    }

    fn golden_intent() -> BootstrapIntent {
        BootstrapIntent {
            root_device: 0x0102_0304_0506_0708,
            root_inode: 0x1112_1314_1516_1718,
            owner_uid: 501,
            library_id: golden_library_id(),
            created_at: golden_timestamp(),
            migration_digest: Sha256Digest::from_bytes(core::array::from_fn(|index| index as u8)),
        }
    }

    fn golden_library_id() -> Id<LibraryIdentity> {
        Id::from_bytes([
            0x01, 0x89, 0x0f, 0x1d, 0xe0, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01,
        ])
        .expect("golden UUIDv7")
    }

    fn golden_timestamp() -> Timestamp {
        Timestamp::from_unix_seconds_nanos(1_700_000_000, 123_456_789).expect("golden timestamp")
    }

    fn golden_record() -> [u8; BOOTSTRAP_INTENT_LENGTH] {
        assert_eq!(GOLDEN_HEX.len(), BOOTSTRAP_INTENT_LENGTH * 2);
        let mut bytes = [0_u8; BOOTSTRAP_INTENT_LENGTH];
        let (pairs, remainder) = GOLDEN_HEX.as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        for (target, pair) in bytes.iter_mut().zip(pairs) {
            *target = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
        }
        bytes
    }

    fn hex_nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("invalid golden hex"),
        }
    }

    fn reference_sha256(input: &[u8]) -> [u8; 32] {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut state = [
            0x6a09e667_u32,
            0xbb67ae85,
            0x3c6ef372,
            0xa54ff53a,
            0x510e527f,
            0x9b05688c,
            0x1f83d9ab,
            0x5be0cd19,
        ];
        let bit_length = (input.len() as u64) * 8;
        let mut padded = input.to_vec();
        padded.push(0x80);
        while padded.len() % 64 != 56 {
            padded.push(0);
        }
        padded.extend_from_slice(&bit_length.to_be_bytes());

        let (chunks, remainder) = padded.as_slice().as_chunks::<64>();
        assert!(remainder.is_empty());
        for chunk in chunks {
            let mut words = [0_u32; 64];
            let (input_words, remainder) = chunk.as_slice().as_chunks::<4>();
            assert!(remainder.is_empty());
            for (target, bytes) in words[..16].iter_mut().zip(input_words) {
                *target = u32::from_be_bytes(*bytes);
            }
            for index in 16..64 {
                let s0 = words[index - 15].rotate_right(7)
                    ^ words[index - 15].rotate_right(18)
                    ^ (words[index - 15] >> 3);
                let s1 = words[index - 2].rotate_right(17)
                    ^ words[index - 2].rotate_right(19)
                    ^ (words[index - 2] >> 10);
                words[index] = words[index - 16]
                    .wrapping_add(s0)
                    .wrapping_add(words[index - 7])
                    .wrapping_add(s1);
            }
            let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
            for index in 0..64 {
                let big_sigma_one = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let choose = (e & f) ^ ((!e) & g);
                let temporary_one = h
                    .wrapping_add(big_sigma_one)
                    .wrapping_add(choose)
                    .wrapping_add(K[index])
                    .wrapping_add(words[index]);
                let big_sigma_zero = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let majority = (a & b) ^ (a & c) ^ (b & c);
                let temporary_two = big_sigma_zero.wrapping_add(majority);
                h = g;
                g = f;
                f = e;
                e = d.wrapping_add(temporary_one);
                d = c;
                c = b;
                b = a;
                a = temporary_one.wrapping_add(temporary_two);
            }
            for (target, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
                *target = target.wrapping_add(value);
            }
        }

        let mut digest = [0_u8; 32];
        let (digest_words, remainder) = digest.as_mut_slice().as_chunks_mut::<4>();
        assert!(remainder.is_empty());
        for (bytes, value) in digest_words.iter_mut().zip(state) {
            bytes.copy_from_slice(&value.to_be_bytes());
        }
        digest
    }
}
