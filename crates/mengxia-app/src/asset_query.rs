use std::sync::Arc;

use mengxia_domain::{Asset, AssetRevision, Location, Representation, Resource};
use mengxia_ports::{
    AssetMemberPage, AssetPage, AssetQueryPort, AssetStoreError, IngestControl, IngestDirective,
    InspectAssetPosition, InspectAssetQuery, InspectAssetStart, InspectMemberPhase,
    InterruptibleSqliteControl, ListAssetsPosition, ListAssetsQuery, SqliteInterrupt,
    SqliteInterruptControlError,
};
use mengxia_types::{Id, RevisionNo};
use sha2::{Digest as _, Sha256};

const LIST_CURSOR_LENGTH: usize = 80;
const LIST_CURSOR_PREFIX_LENGTH: usize = 48;
const LIST_CURSOR_MAGIC: [u8; 8] = *b"MXLCUR2\0";
const CURSOR_FORMAT_VERSION: u16 = 2;
const LIST_OPERATION_DISCRIMINATOR: u32 = 1;
const INSPECT_CURSOR_LENGTH: usize = 208;
const INSPECT_CURSOR_PREFIX_LENGTH: usize = 176;
const INSPECT_CURSOR_MAGIC: [u8; 8] = *b"MXICUR2\0";
const INSPECT_OPERATION_DISCRIMINATOR: u32 = 2;

/// Validates only the checksum of an opaque TASK-008 cursor.
///
/// This exposes neither decoded cursor fields nor authority-bearing state. A
/// transport client can use it to reject corrupted server output before display.
#[must_use]
pub fn opaque_cursor_checksum_is_valid(cursor: &[u8]) -> bool {
    const CHECKSUM_LENGTH: usize = 32;

    cursor.len() >= CHECKSUM_LENGTH
        && Sha256::digest(&cursor[..cursor.len() - CHECKSUM_LENGTH]).as_slice()
            == &cursor[cursor.len() - CHECKSUM_LENGTH..]
}

struct ContinueSqliteControl;

impl IngestControl for ContinueSqliteControl {
    fn checkpoint(&self) -> IngestDirective {
        IngestDirective::Continue
    }
}

impl InterruptibleSqliteControl for ContinueSqliteControl {
    fn register_interrupt(
        &self,
        _interrupt: Box<dyn SqliteInterrupt>,
    ) -> Result<IngestDirective, SqliteInterruptControlError> {
        Ok(IngestDirective::Continue)
    }

    fn clear_interrupt(&self) -> Result<(), SqliteInterruptControlError> {
        Ok(())
    }
}

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

pub struct InspectAssetResponse {
    page: AssetMemberPage,
    next_cursor: Option<[u8; INSPECT_CURSOR_LENGTH]>,
}

impl InspectAssetResponse {
    #[must_use]
    pub const fn page(&self) -> &AssetMemberPage {
        &self.page
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&[u8; INSPECT_CURSOR_LENGTH]> {
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
        self.list_assets_controlled(page_size, cursor, Arc::new(ContinueSqliteControl))
            .await
    }

    pub async fn list_assets_controlled(
        &self,
        page_size: u32,
        cursor: Option<&[u8]>,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<ListAssetsResponse, AssetStoreError> {
        query_checkpoint(&control)?;
        let position = match cursor {
            None | Some([]) => ListAssetsPosition::First,
            Some(cursor) => decode_list_cursor(cursor, self.library_id)?,
        };
        let page = self
            .port
            .list_assets(ListAssetsQuery::new(page_size, position)?, control)
            .await?;
        let next_cursor = page
            .next()
            .map(|position| encode_list_cursor(position, self.library_id))
            .transpose()?;
        Ok(ListAssetsResponse { page, next_cursor })
    }

    pub async fn inspect_asset(
        &self,
        asset_id: Id<Asset>,
        selected_revision_id: Option<Id<AssetRevision>>,
        page_size: u32,
        cursor: Option<&[u8]>,
    ) -> Result<InspectAssetResponse, AssetStoreError> {
        self.inspect_asset_controlled(
            asset_id,
            selected_revision_id,
            page_size,
            cursor,
            Arc::new(ContinueSqliteControl),
        )
        .await
    }

    pub async fn inspect_asset_controlled(
        &self,
        asset_id: Id<Asset>,
        selected_revision_id: Option<Id<AssetRevision>>,
        page_size: u32,
        cursor: Option<&[u8]>,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<InspectAssetResponse, AssetStoreError> {
        query_checkpoint(&control)?;
        let start = match cursor {
            None | Some([]) => InspectAssetStart::First,
            Some(cursor) => {
                InspectAssetStart::Continue(decode_inspect_cursor(cursor, self.library_id)?)
            }
        };
        let page = self
            .port
            .inspect_asset(
                InspectAssetQuery::new(asset_id, selected_revision_id, page_size, start)?,
                control,
            )
            .await?;
        let next_cursor = page
            .next()
            .map(|position| encode_inspect_cursor(position, self.library_id))
            .transpose()?;
        Ok(InspectAssetResponse { page, next_cursor })
    }
}

fn query_checkpoint(control: &Arc<dyn InterruptibleSqliteControl>) -> Result<(), AssetStoreError> {
    match control.checkpoint() {
        IngestDirective::Continue => Ok(()),
        IngestDirective::Stop(mengxia_ports::IngestStop::Cancelled) => {
            Err(AssetStoreError::OperationCancelled)
        }
        IngestDirective::Stop(mengxia_ports::IngestStop::DeadlineReached) => {
            Err(AssetStoreError::DeadlineExceeded)
        }
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

fn decode_inspect_cursor(
    cursor: &[u8],
    expected_library_id: [u8; 16],
) -> Result<InspectAssetPosition, AssetStoreError> {
    let cursor: &[u8; INSPECT_CURSOR_LENGTH] =
        cursor.try_into().map_err(|_| AssetStoreError::Validation)?;
    if cursor[..8] != INSPECT_CURSOR_MAGIC
        || u16::from_be_bytes(cursor[8..10].try_into().expect("fixed cursor range"))
            != CURSOR_FORMAT_VERSION
        || usize::from(u16::from_be_bytes(
            cursor[10..12].try_into().expect("fixed cursor range"),
        )) != INSPECT_CURSOR_LENGTH
        || u32::from_be_bytes(cursor[12..16].try_into().expect("fixed cursor range"))
            != INSPECT_OPERATION_DISCRIMINATOR
        || cursor[16..32] != expected_library_id
        || cursor[136..176].iter().any(|byte| *byte != 0)
        || Sha256::digest(&cursor[..INSPECT_CURSOR_PREFIX_LENGTH]).as_slice()
            != &cursor[INSPECT_CURSOR_PREFIX_LENGTH..]
    {
        return Err(AssetStoreError::Validation);
    }
    let asset_id = typed_id::<Asset>(&cursor[32..48])?;
    let selected_revision_id = typed_id::<AssetRevision>(&cursor[48..64])?;
    let asset_revision = RevisionNo::new(u64::from_be_bytes(
        cursor[64..72].try_into().expect("fixed cursor range"),
    ));
    let phase = u32::from_be_bytes(cursor[108..112].try_into().expect("fixed cursor range"));
    if phase == 0 {
        if cursor[72..136].iter().any(|byte| *byte != 0) {
            return Err(AssetStoreError::Validation);
        }
        return InspectAssetPosition::new(
            expected_library_id,
            asset_id,
            selected_revision_id,
            asset_revision,
            None,
            None,
            None,
            None,
        );
    }
    let phase = match phase {
        1 => InspectMemberPhase::NoLocationSeen,
        2 => InspectMemberPhase::LocationSeen,
        3 => InspectMemberPhase::RequiredCustodySeen,
        _ => return Err(AssetStoreError::Validation),
    };
    let representation_id = typed_id::<Representation>(&cursor[72..88])?;
    let resource_id = typed_id::<Resource>(&cursor[88..104])?;
    let ordinal = u32::from_be_bytes(cursor[104..108].try_into().expect("fixed cursor range"));
    if ordinal > 4095 {
        return Err(AssetStoreError::Validation);
    }
    let blob_revision = RevisionNo::new(u64::from_be_bytes(
        cursor[112..120].try_into().expect("fixed cursor range"),
    ));
    let last_location_id = if cursor[120..136].iter().all(|byte| *byte == 0) {
        None
    } else {
        Some(typed_id::<Location>(&cursor[120..136])?)
    };
    InspectAssetPosition::new(
        expected_library_id,
        asset_id,
        selected_revision_id,
        asset_revision,
        Some((representation_id, resource_id, ordinal)),
        Some(phase),
        Some(blob_revision),
        last_location_id,
    )
}

fn encode_inspect_cursor(
    position: InspectAssetPosition,
    expected_library_id: [u8; 16],
) -> Result<[u8; INSPECT_CURSOR_LENGTH], AssetStoreError> {
    if position.library_id() != expected_library_id {
        return Err(AssetStoreError::StorageCorruption);
    }
    let mut cursor = [0_u8; INSPECT_CURSOR_LENGTH];
    cursor[..8].copy_from_slice(&INSPECT_CURSOR_MAGIC);
    cursor[8..10].copy_from_slice(&CURSOR_FORMAT_VERSION.to_be_bytes());
    cursor[10..12].copy_from_slice(&(INSPECT_CURSOR_LENGTH as u16).to_be_bytes());
    cursor[12..16].copy_from_slice(&INSPECT_OPERATION_DISCRIMINATOR.to_be_bytes());
    cursor[16..32].copy_from_slice(&expected_library_id);
    cursor[32..48].copy_from_slice(&position.asset_id().to_bytes());
    cursor[48..64].copy_from_slice(&position.selected_revision_id().to_bytes());
    cursor[64..72].copy_from_slice(&position.asset_revision().get().to_be_bytes());
    match (
        position.member(),
        position.phase(),
        position.blob_revision(),
    ) {
        (None, None, None) if position.last_location_id().is_none() => {}
        (Some((representation_id, resource_id, ordinal)), Some(phase), Some(blob_revision)) => {
            cursor[72..88].copy_from_slice(&representation_id.to_bytes());
            cursor[88..104].copy_from_slice(&resource_id.to_bytes());
            cursor[104..108].copy_from_slice(&ordinal.to_be_bytes());
            cursor[108..112].copy_from_slice(&(phase as u32).to_be_bytes());
            cursor[112..120].copy_from_slice(&blob_revision.get().to_be_bytes());
            if let Some(location_id) = position.last_location_id() {
                cursor[120..136].copy_from_slice(&location_id.to_bytes());
            }
        }
        _ => return Err(AssetStoreError::Internal),
    }
    let checksum: [u8; 32] = Sha256::digest(&cursor[..INSPECT_CURSOR_PREFIX_LENGTH]).into();
    cursor[INSPECT_CURSOR_PREFIX_LENGTH..].copy_from_slice(&checksum);
    Ok(cursor)
}

fn typed_id<T>(bytes: &[u8]) -> Result<Id<T>, AssetStoreError> {
    Id::from_bytes(bytes.try_into().map_err(|_| AssetStoreError::Validation)?)
        .map_err(|_| AssetStoreError::Validation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_id<T>(tail: u8) -> Id<T> {
        let mut bytes = [
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x00,
        ];
        bytes[15] = tail;
        Id::from_bytes(bytes).expect("fixed UUIDv7")
    }

    #[test]
    fn list_cursor_round_trip_and_corruption_checks_are_exact() {
        let library_id = [0x11; 16];
        let position = ListAssetsPosition::after(library_id, 0x0102_0304, 7).unwrap();
        let encoded = encode_list_cursor(position, library_id).unwrap();
        assert_eq!(encoded.len(), LIST_CURSOR_LENGTH);
        assert_eq!(&encoded[..8], b"MXLCUR2\0");
        assert_eq!(
            encoded
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "4d584c43555232000002005000000001111111111111111111111111111111110000000001020304000000000000000758265fa68a08c54cb8eaea6736e9f3bd66cc5b8e4fb2676c3a56a94b990c79dd"
        );
        assert!(opaque_cursor_checksum_is_valid(&encoded));
        assert_eq!(decode_list_cursor(&encoded, library_id), Ok(position));

        for index in [0, 8, 10, 12, 16, 32, 40, 47, 48, 79] {
            let mut corrupted = encoded;
            corrupted[index] ^= 1;
            assert!(!opaque_cursor_checksum_is_valid(&corrupted));
            assert!(decode_list_cursor(&corrupted, library_id).is_err());
        }
        assert!(decode_list_cursor(&encoded, [0x22; 16]).is_err());
        assert!(decode_list_cursor(&encoded[..79], library_id).is_err());
        assert!(encode_list_cursor(ListAssetsPosition::First, library_id).is_err());
    }

    #[test]
    fn inspect_cursor_round_trip_and_phase_rules_are_exact() {
        let library_id = [0x11; 16];
        let asset_id = fixed_id::<Asset>(1);
        let revision_id = fixed_id::<AssetRevision>(2);
        let representation_id = fixed_id::<Representation>(3);
        let resource_id = fixed_id::<Resource>(4);
        let location_id = fixed_id::<Location>(5);
        let position = InspectAssetPosition::new(
            library_id,
            asset_id,
            revision_id,
            RevisionNo::new(9),
            Some((representation_id, resource_id, 4095)),
            Some(InspectMemberPhase::RequiredCustodySeen),
            Some(RevisionNo::new(4)),
            Some(location_id),
        )
        .unwrap();
        let encoded = encode_inspect_cursor(position, library_id).unwrap();
        assert_eq!(
            encoded
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "4d58494355523200000200d00000000211111111111111111111111111111111018d442fc0007a118022334455667701018d442fc0007a1180223344556677020000000000000009018d442fc0007a118022334455667703018d442fc0007a11802233445566770400000fff000000030000000000000004018d442fc0007a118022334455667705000000000000000000000000000000000000000000000000000000000000000000000000000000007b2e881fdeb88ee467926e1cc197330c889f69f6c6d789d7224e3bc6f39df810"
        );
        assert!(opaque_cursor_checksum_is_valid(&encoded));
        assert_eq!(decode_inspect_cursor(&encoded, library_id), Ok(position));
        for index in [
            0, 8, 10, 12, 16, 64, 72, 104, 108, 112, 120, 136, 175, 176, 207,
        ] {
            let mut corrupted = encoded;
            corrupted[index] ^= 1;
            assert!(!opaque_cursor_checksum_is_valid(&corrupted));
            assert!(decode_inspect_cursor(&corrupted, library_id).is_err());
        }

        let before_first = InspectAssetPosition::new(
            library_id,
            asset_id,
            revision_id,
            RevisionNo::new(9),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let encoded = encode_inspect_cursor(before_first, library_id).unwrap();
        assert_eq!(
            decode_inspect_cursor(&encoded, library_id),
            Ok(before_first)
        );
        assert!(
            InspectAssetPosition::new(
                library_id,
                asset_id,
                revision_id,
                RevisionNo::new(9),
                Some((representation_id, resource_id, 0)),
                Some(InspectMemberPhase::NoLocationSeen),
                Some(RevisionNo::new(4)),
                Some(location_id),
            )
            .is_err()
        );
        assert!(
            InspectAssetPosition::new(
                library_id,
                asset_id,
                revision_id,
                RevisionNo::new(9),
                Some((representation_id, resource_id, 0)),
                Some(InspectMemberPhase::LocationSeen),
                Some(RevisionNo::new(4)),
                None,
            )
            .is_err()
        );
    }
}
