use mengxia_platform_fs::{OpenedLibraryAuthority, ValidatedSqliteWal};

use super::StoreError;
use super::error::map_authority_error;

const WAL_HEADER_LENGTH: usize = 32;
const WAL_FRAME_HEADER_LENGTH: usize = 24;
const WAL_MAGIC_LITTLE_CHECKSUM: u32 = 0x377f_0682;
const WAL_MAGIC_BIG_CHECKSUM: u32 = 0x377f_0683;
const WAL_FORMAT_VERSION: u32 = 3_007_000;
// TASK-004 staging can contain only one small, static bootstrap transaction.
// This conservative ceiling keeps corrupt owner-only input from causing an
// unbounded pre-open scan while remaining far above the pinned schema's needs.
const MAX_BOOTSTRAP_FRAMES_BEFORE_COMMIT: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BootstrapWalEvidence {
    AbsentOrUncommitted,
    Committed,
    CorruptBeforeCommit,
}

pub(crate) fn inspect_bootstrap_wal(
    authority: &OpenedLibraryAuthority,
) -> Result<BootstrapWalEvidence, StoreError> {
    let Some(mut wal) = authority
        .open_bootstrap_staging_wal()
        .map_err(map_authority_error)?
    else {
        return Ok(BootstrapWalEvidence::AbsentOrUncommitted);
    };
    inspect_wal(&mut wal).map_err(map_authority_error)
}

fn inspect_wal(
    wal: &mut ValidatedSqliteWal,
) -> Result<BootstrapWalEvidence, mengxia_platform_fs::AuthorityError> {
    let mut header = [0_u8; WAL_HEADER_LENGTH];
    match read_exact_or_eof(wal, &mut header)? {
        ReadResult::CleanEof => {
            return Ok(BootstrapWalEvidence::AbsentOrUncommitted);
        }
        ReadResult::Partial => return Ok(BootstrapWalEvidence::CorruptBeforeCommit),
        ReadResult::Complete => {}
    }

    let magic = be_u32(&header[0..4]);
    let checksum_endian = match magic {
        WAL_MAGIC_LITTLE_CHECKSUM => ChecksumEndian::Little,
        WAL_MAGIC_BIG_CHECKSUM => ChecksumEndian::Big,
        _ => return Ok(BootstrapWalEvidence::CorruptBeforeCommit),
    };
    if be_u32(&header[4..8]) != WAL_FORMAT_VERSION {
        return Ok(BootstrapWalEvidence::CorruptBeforeCommit);
    }
    let page_size = be_u32(&header[8..12]) as usize;
    if !(512..=65_536).contains(&page_size) || !page_size.is_power_of_two() {
        return Ok(BootstrapWalEvidence::CorruptBeforeCommit);
    }

    let mut checksum = checksum_bytes(checksum_endian, &header[..24], [0, 0]);
    if checksum != [be_u32(&header[24..28]), be_u32(&header[28..32])] {
        return Ok(BootstrapWalEvidence::CorruptBeforeCommit);
    }
    let salt = [be_u32(&header[16..20]), be_u32(&header[20..24])];
    let mut frame = vec![0_u8; WAL_FRAME_HEADER_LENGTH + page_size];
    for _ in 0..MAX_BOOTSTRAP_FRAMES_BEFORE_COMMIT {
        match read_exact_or_eof(wal, &mut frame)? {
            ReadResult::CleanEof => {
                return Ok(BootstrapWalEvidence::AbsentOrUncommitted);
            }
            ReadResult::Partial => {
                return Ok(BootstrapWalEvidence::CorruptBeforeCommit);
            }
            ReadResult::Complete => {}
        }

        let frame_salt = [be_u32(&frame[8..12]), be_u32(&frame[12..16])];
        let page_number = be_u32(&frame[0..4]);
        let mut next_checksum = checksum_bytes(checksum_endian, &frame[..8], checksum);
        next_checksum = checksum_bytes(
            checksum_endian,
            &frame[WAL_FRAME_HEADER_LENGTH..],
            next_checksum,
        );
        let stored_checksum = [be_u32(&frame[16..20]), be_u32(&frame[20..24])];
        if frame_salt != salt || page_number == 0 || next_checksum != stored_checksum {
            return Ok(BootstrapWalEvidence::CorruptBeforeCommit);
        }
        checksum = next_checksum;
        if be_u32(&frame[4..8]) != 0 {
            return Ok(BootstrapWalEvidence::Committed);
        }
    }
    Ok(BootstrapWalEvidence::CorruptBeforeCommit)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadResult {
    Complete,
    CleanEof,
    Partial,
}

fn read_exact_or_eof(
    wal: &mut ValidatedSqliteWal,
    buffer: &mut [u8],
) -> Result<ReadResult, mengxia_platform_fs::AuthorityError> {
    let mut read = 0;
    while read < buffer.len() {
        let count = wal.read_chunk(&mut buffer[read..])?;
        if count == 0 {
            return Ok(if read == 0 {
                ReadResult::CleanEof
            } else {
                ReadResult::Partial
            });
        }
        read += count;
    }
    Ok(ReadResult::Complete)
}

#[derive(Clone, Copy)]
enum ChecksumEndian {
    Little,
    Big,
}

fn checksum_bytes(endian: ChecksumEndian, bytes: &[u8], initial: [u32; 2]) -> [u32; 2] {
    debug_assert!(bytes.len() >= 8 && bytes.len().is_multiple_of(8));
    let mut first = initial[0];
    let mut second = initial[1];
    for pair in bytes.as_chunks::<8>().0 {
        first = first
            .wrapping_add(checksum_u32(endian, &pair[..4]))
            .wrapping_add(second);
        second = second
            .wrapping_add(checksum_u32(endian, &pair[4..]))
            .wrapping_add(first);
    }
    [first, second]
}

fn checksum_u32(endian: ChecksumEndian, bytes: &[u8]) -> u32 {
    let bytes: [u8; 4] = bytes.try_into().expect("checksum words are four bytes");
    match endian {
        ChecksumEndian::Little => u32::from_le_bytes(bytes),
        ChecksumEndian::Big => u32::from_be_bytes(bytes),
    }
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("WAL integers are four bytes"))
}
