use std::sync::Arc;

use mengxia_ports::{
    AssetPage, AssetQueryPort, AssetStoreError, ListAssetsPosition, ListAssetsQuery,
};
use sha2::{Digest as _, Sha256};

const LIST_CURSOR_LENGTH: usize = 80;
const LIST_CURSOR_PREFIX_LENGTH: usize = 48;
const LIST_CURSOR_MAGIC: [u8; 8] = *b"MXLCUR1\0";
const CURSOR_FORMAT_VERSION: u16 = 1;
const LIST_OPERATION_DISCRIMINATOR: u32 = 1;

/// Application-owned bounded ListAssets response with an opaque wire cursor.
pub struct ListAssetsResponse {
    page: AssetPage,
    next_cursor: Option<[u8; LIST_CURSOR_LENGTH]>,
}

impl ListAssetsResponse {
    #[must_use]
    pub const fn page(&self) -> &AssetPage {
        &self.page
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&[u8; LIST_CURSOR_LENGTH]> {
        self.next_cursor.as_ref()
    }
}

/// Stateless query orchestration; cursor bytes never cross the storage port.
pub struct AssetQueryService<P> {
    port: Arc<P>,
    library_id: [u8; 16],
}

impl<P> AssetQueryService<P>
where
    P: AssetQueryPort,
{
    pub fn new(port: Arc<P>, library_id: [u8; 16]) -> Result<Self, AssetStoreError> {
        if library_id == [0; 16] {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self { port, library_id })
    }

    pub async fn list_assets(
        &self,
        page_size: u32,
        cursor: Option<&[u8]>,
    ) -> Result<ListAssetsResponse, AssetStoreError> {
        let position = match cursor {
            None | Some([]) => ListAssetsPosition::First,
            Some(cursor) => decode_list_cursor(cursor, self.library_id)?,
        };
        let page = self
            .port
            .list_assets(ListAssetsQuery::new(page_size, position)?)
            .await?;
        let next_cursor = page
            .next()
            .map(|position| encode_list_cursor(position, self.library_id))
            .transpose()?;
        Ok(ListAssetsResponse { page, next_cursor })
    }
}

fn decode_list_cursor(
    cursor: &[u8],
    expected_library_id: [u8; 16],
) -> Result<ListAssetsPosition, AssetStoreError> {
    let cursor: &[u8; LIST_CURSOR_LENGTH] =
        cursor.try_into().map_err(|_| AssetStoreError::Validation)?;
    if cursor[..8] != LIST_CURSOR_MAGIC
        || u16::from_be_bytes(cursor[8..10].try_into().expect("fixed cursor range"))
            != CURSOR_FORMAT_VERSION
        || usize::from(u16::from_be_bytes(
            cursor[10..12].try_into().expect("fixed cursor range"),
        )) != LIST_CURSOR_LENGTH
        || u32::from_be_bytes(cursor[12..16].try_into().expect("fixed cursor range"))
            != LIST_OPERATION_DISCRIMINATOR
        || cursor[16..32] != expected_library_id
        || Sha256::digest(&cursor[..LIST_CURSOR_PREFIX_LENGTH]).as_slice()
            != &cursor[LIST_CURSOR_PREFIX_LENGTH..]
    {
        return Err(AssetStoreError::Validation);
    }
    let snapshot = u64::from_be_bytes(cursor[32..40].try_into().expect("fixed cursor range"));
    let last_examined = u64::from_be_bytes(cursor[40..48].try_into().expect("fixed cursor range"));
    ListAssetsPosition::after(expected_library_id, snapshot, last_examined)
}

fn encode_list_cursor(
    position: ListAssetsPosition,
    expected_library_id: [u8; 16],
) -> Result<[u8; LIST_CURSOR_LENGTH], AssetStoreError> {
    let ListAssetsPosition::After {
        library_id,
        snapshot_sequence,
        last_examined_sequence,
    } = position
    else {
        return Err(AssetStoreError::Internal);
    };
    if library_id != expected_library_id {
        return Err(AssetStoreError::StorageCorruption);
    }
    ListAssetsPosition::after(library_id, snapshot_sequence, last_examined_sequence)?;
    let mut cursor = [0_u8; LIST_CURSOR_LENGTH];
    cursor[..8].copy_from_slice(&LIST_CURSOR_MAGIC);
    cursor[8..10].copy_from_slice(&CURSOR_FORMAT_VERSION.to_be_bytes());
    cursor[10..12].copy_from_slice(&(LIST_CURSOR_LENGTH as u16).to_be_bytes());
    cursor[12..16].copy_from_slice(&LIST_OPERATION_DISCRIMINATOR.to_be_bytes());
    cursor[16..32].copy_from_slice(&library_id);
    cursor[32..40].copy_from_slice(&snapshot_sequence.to_be_bytes());
    cursor[40..48].copy_from_slice(&last_examined_sequence.to_be_bytes());
    let checksum: [u8; 32] = Sha256::digest(&cursor[..LIST_CURSOR_PREFIX_LENGTH]).into();
    cursor[LIST_CURSOR_PREFIX_LENGTH..].copy_from_slice(&checksum);
    Ok(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_cursor_round_trip_and_corruption_checks_are_exact() {
        let library_id = [0x11; 16];
        let position = ListAssetsPosition::after(library_id, 0x0102_0304, 7).unwrap();
        let encoded = encode_list_cursor(position, library_id).unwrap();
        assert_eq!(encoded.len(), LIST_CURSOR_LENGTH);
        assert_eq!(&encoded[..8], b"MXLCUR1\0");
        assert_eq!(
            encoded
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "4d584c4355523100000100500000000111111111111111111111111111111111000000000102030400000000000000075207108154fece31201183425baa5902b69ad81f1250f97e8116e4513deb6614"
        );
        assert_eq!(decode_list_cursor(&encoded, library_id), Ok(position));

        for index in [0, 8, 10, 12, 16, 32, 40, 47, 48, 79] {
            let mut corrupted = encoded;
            corrupted[index] ^= 1;
            assert!(decode_list_cursor(&corrupted, library_id).is_err());
        }
        assert!(decode_list_cursor(&encoded, [0x22; 16]).is_err());
        assert!(decode_list_cursor(&encoded[..79], library_id).is_err());
        assert!(encode_list_cursor(ListAssetsPosition::First, library_id).is_err());
    }
}
