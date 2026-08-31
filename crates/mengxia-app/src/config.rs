use mengxia_types::ErrorCode;

const HEADER: &[u8] = b"MENGXIA_LIBRARY_CONFIG_V1\n";
const KEY_COUNT: usize = 22;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LibraryConfigKey {
    BlobRoot,
    ClientEndpoint,
    ClientHandshakeTimeoutMs,
    ClientOperationTimeoutMs,
    DbBusyTimeoutMs,
    DbReadConnections,
    DbWriteQueue,
    HashConcurrency,
    IngestShutdownTimeoutMs,
    LibraryRoot,
    MaxClientSessions,
    MaxConcurrentIngests,
    MaxDecodeDepth,
    MaxFrameBytes,
    MaxIngestBytes,
    MaxIngestOperationTimeoutMs,
    MaxPendingHandshakes,
    MaxStagingBytes,
    MinFreeBytes,
    MinFreePercent,
    StorageIoConcurrency,
    StreamBufferBytes,
}

impl LibraryConfigKey {
    const fn index(self) -> usize {
        self as usize
    }

    fn parse(value: &[u8]) -> Option<Self> {
        Some(match value {
            b"MENGXIA_BLOB_ROOT" => Self::BlobRoot,
            b"MENGXIA_CLIENT_ENDPOINT" => Self::ClientEndpoint,
            b"MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS" => Self::ClientHandshakeTimeoutMs,
            b"MENGXIA_CLIENT_OPERATION_TIMEOUT_MS" => Self::ClientOperationTimeoutMs,
            b"MENGXIA_DB_BUSY_TIMEOUT_MS" => Self::DbBusyTimeoutMs,
            b"MENGXIA_DB_READ_CONNECTIONS" => Self::DbReadConnections,
            b"MENGXIA_DB_WRITE_QUEUE" => Self::DbWriteQueue,
            b"MENGXIA_HASH_CONCURRENCY" => Self::HashConcurrency,
            b"MENGXIA_INGEST_SHUTDOWN_TIMEOUT_MS" => Self::IngestShutdownTimeoutMs,
            b"MENGXIA_LIBRARY_ROOT" => Self::LibraryRoot,
            b"MENGXIA_MAX_CLIENT_SESSIONS" => Self::MaxClientSessions,
            b"MENGXIA_MAX_CONCURRENT_INGESTS" => Self::MaxConcurrentIngests,
            b"MENGXIA_MAX_DECODE_DEPTH" => Self::MaxDecodeDepth,
            b"MENGXIA_MAX_FRAME_BYTES" => Self::MaxFrameBytes,
            b"MENGXIA_MAX_INGEST_BYTES" => Self::MaxIngestBytes,
            b"MENGXIA_MAX_INGEST_OPERATION_TIMEOUT_MS" => Self::MaxIngestOperationTimeoutMs,
            b"MENGXIA_MAX_PENDING_HANDSHAKES" => Self::MaxPendingHandshakes,
            b"MENGXIA_MAX_STAGING_BYTES" => Self::MaxStagingBytes,
            b"MENGXIA_MIN_FREE_BYTES" => Self::MinFreeBytes,
            b"MENGXIA_MIN_FREE_PERCENT" => Self::MinFreePercent,
            b"MENGXIA_STORAGE_IO_CONCURRENCY" => Self::StorageIoConcurrency,
            b"MENGXIA_STREAM_BUFFER_BYTES" => Self::StreamBufferBytes,
            _ => return None,
        })
    }
}

pub struct LibraryConfigDocument {
    values: [Option<Vec<u8>>; KEY_COUNT],
}

impl LibraryConfigDocument {
    pub fn parse(bytes: &[u8]) -> Result<Self, ErrorCode> {
        if bytes.len() > 16_384
            || !bytes.starts_with(HEADER)
            || !bytes.ends_with(b"\n")
            || bytes.contains(&0)
            || bytes.contains(&b'\r')
        {
            return Err(ErrorCode::ValidationError);
        }
        let body = &bytes[HEADER.len()..];
        let lines = if body.is_empty() {
            &[][..]
        } else {
            &body[..body.len() - 1]
        };
        if lines.is_empty() && !body.is_empty() {
            return Err(ErrorCode::ValidationError);
        }
        let mut values: [Option<Vec<u8>>; KEY_COUNT] = std::array::from_fn(|_| None);
        let mut previous: Option<&[u8]> = None;
        let mut count = 0_usize;
        if !lines.is_empty() {
            for line in lines.split(|byte| *byte == b'\n') {
                count += 1;
                if count > 64
                    || !(3..=2048).contains(&line.len())
                    || line.iter().any(|byte| *byte < 0x20 || *byte == 0x7f)
                {
                    return Err(ErrorCode::ValidationError);
                }
                let separator = line
                    .iter()
                    .position(|byte| *byte == b'=')
                    .ok_or(ErrorCode::ValidationError)?;
                let key_bytes = &line[..separator];
                let value = &line[separator + 1..];
                if value.is_empty() || previous.is_some_and(|last| last >= key_bytes) {
                    return Err(ErrorCode::ValidationError);
                }
                let key = LibraryConfigKey::parse(key_bytes).ok_or(ErrorCode::ValidationError)?;
                if values[key.index()].is_some() {
                    return Err(ErrorCode::ValidationError);
                }
                values[key.index()] = Some(value.to_vec());
                previous = Some(key_bytes);
            }
        }
        Ok(Self { values })
    }

    #[must_use]
    pub fn value(&self, key: LibraryConfigKey) -> Option<&[u8]> {
        self.values[key.index()].as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::{LibraryConfigDocument, LibraryConfigKey};

    #[test]
    fn exact_sorted_byte_preserving_document_parses() {
        let bytes = b"MENGXIA_LIBRARY_CONFIG_V1\nMENGXIA_BLOB_ROOT=/Users/me/Blob\nMENGXIA_MAX_FRAME_BYTES=65536\n";
        let document = LibraryConfigDocument::parse(bytes).unwrap();
        assert_eq!(
            document.value(LibraryConfigKey::BlobRoot),
            Some(&b"/Users/me/Blob"[..])
        );
        assert_eq!(
            document.value(LibraryConfigKey::MaxFrameBytes),
            Some(&b"65536"[..])
        );
    }

    #[test]
    fn malformed_unknown_duplicate_unsorted_or_control_input_fails() {
        for bytes in [
            &b"bad\n"[..],
            &b"MENGXIA_LIBRARY_CONFIG_V1\nUNKNOWN=x\n"[..],
            &b"MENGXIA_LIBRARY_CONFIG_V1\nMENGXIA_MAX_FRAME_BYTES=1\nMENGXIA_BLOB_ROOT=/x\n"[..],
            &b"MENGXIA_LIBRARY_CONFIG_V1\nMENGXIA_BLOB_ROOT=/x\r\n"[..],
            &b"MENGXIA_LIBRARY_CONFIG_V1\n\n"[..],
        ] {
            assert!(LibraryConfigDocument::parse(bytes).is_err());
        }
    }
}
