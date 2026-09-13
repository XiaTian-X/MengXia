use std::fmt;

/// Exact minimum capable of decoding an envelope and its body.
pub const MIN_DECODE_DEPTH: u8 = 2;
/// Accepted hard ceiling for TASK-011 wire depth.
pub const MAX_DECODE_DEPTH: u8 = 16;

/// Validated maximum embedded-message depth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeDepth(u8);

impl DecodeDepth {
    pub const fn new(value: u8) -> Result<Self, ProtocolCodecError> {
        if value < MIN_DECODE_DEPTH || value > MAX_DECODE_DEPTH {
            return Err(ProtocolCodecError::InvalidLimit);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Redacted closed-codec failure; it never retains peer bytes or values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtocolCodecError {
    InvalidLimit,
    UnknownOrReservedField,
    DuplicateField,
    NonCanonicalEncoding,
    NonCanonicalVarint,
    WrongWireType,
    UnsupportedGroup,
    Truncated,
    DepthExceeded,
    InvalidSemanticValue,
    AllocationUnavailable,
    InvalidFrameLength,
    Transport,
}

impl fmt::Display for ProtocolCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "invalid protocol limit",
            Self::UnknownOrReservedField => "unknown or reserved protocol field",
            Self::DuplicateField => "duplicate protocol field",
            Self::NonCanonicalEncoding => "non-canonical protocol encoding",
            Self::NonCanonicalVarint => "non-canonical protocol varint",
            Self::WrongWireType => "wrong protocol wire type",
            Self::UnsupportedGroup => "protobuf groups are unsupported",
            Self::Truncated => "truncated protocol value",
            Self::DepthExceeded => "protocol decode depth exceeded",
            Self::InvalidSemanticValue => "invalid protocol value",
            Self::AllocationUnavailable => "protocol allocation unavailable",
            Self::InvalidFrameLength => "invalid protocol frame length",
            Self::Transport => "protocol transport failed",
        })
    }
}

impl std::error::Error for ProtocolCodecError {}

#[derive(Clone, Copy)]
pub(crate) enum MessageKind {
    HostEnvelope,
    PluginEnvelope,
    HostHello,
    PluginHello,
    PingRequest,
    PingResponse,
    ShutdownRequest,
    ShutdownResponse,
    PluginFailure,
}

#[derive(Clone, Copy)]
pub(crate) struct FieldSpec {
    pub(crate) wire: u8,
    pub(crate) child: Option<MessageKind>,
    pub(crate) oneof: bool,
}

include!(concat!(env!("OUT_DIR"), "/plugin_wire_schema.rs"));

pub(crate) fn validate_message(
    input: &[u8],
    root: MessageKind,
    depth: DecodeDepth,
) -> Result<(), ProtocolCodecError> {
    scan_message(input, root, 1, depth.get())
}

fn scan_message(
    input: &[u8],
    kind: MessageKind,
    current_depth: u8,
    maximum_depth: u8,
) -> Result<(), ProtocolCodecError> {
    if current_depth > maximum_depth {
        return Err(ProtocolCodecError::DepthExceeded);
    }
    let mut offset = 0_usize;
    let mut observed = 0_u16;
    let mut observed_oneof = false;
    let mut last_number = 0_u64;
    while offset < input.len() {
        let key = read_varint(input, &mut offset)?;
        let number = key >> 3;
        if number == 0 {
            return Err(ProtocolCodecError::UnknownOrReservedField);
        }
        let wire = u8::try_from(key & 7).map_err(|_| ProtocolCodecError::WrongWireType)?;
        if matches!(wire, 3 | 4) {
            return Err(ProtocolCodecError::UnsupportedGroup);
        }
        let field =
            descriptor_field(kind, number).ok_or(ProtocolCodecError::UnknownOrReservedField)?;
        if number <= last_number {
            return Err(if number == last_number {
                ProtocolCodecError::DuplicateField
            } else {
                ProtocolCodecError::NonCanonicalEncoding
            });
        }
        last_number = number;
        if wire != field.wire {
            return Err(ProtocolCodecError::WrongWireType);
        }
        let bit = 1_u16
            .checked_shl(u32::try_from(number).map_err(|_| ProtocolCodecError::DuplicateField)?)
            .ok_or(ProtocolCodecError::DuplicateField)?;
        if observed & bit != 0 || (field.oneof && observed_oneof) {
            return Err(ProtocolCodecError::DuplicateField);
        }
        observed |= bit;
        observed_oneof |= field.oneof;

        match wire {
            0 => {
                let value = read_varint(input, &mut offset)?;
                if value == 0 {
                    return Err(ProtocolCodecError::NonCanonicalEncoding);
                }
                if matches!(kind, MessageKind::PluginFailure) && number == 1 && value > 3 {
                    return Err(ProtocolCodecError::InvalidSemanticValue);
                }
            }
            1 => {
                let end = advance_end(input, offset, 8)?;
                if input[offset..end].iter().all(|byte| *byte == 0) {
                    return Err(ProtocolCodecError::NonCanonicalEncoding);
                }
                offset = end;
            }
            2 => {
                let length = read_varint(input, &mut offset)?;
                let length = usize::try_from(length)
                    .map_err(|_| ProtocolCodecError::InvalidSemanticValue)?;
                let end = offset
                    .checked_add(length)
                    .filter(|end| *end <= input.len())
                    .ok_or(ProtocolCodecError::Truncated)?;
                if field.child.is_none() && length == 0 {
                    return Err(ProtocolCodecError::NonCanonicalEncoding);
                }
                if let Some(child) = field.child {
                    scan_message(
                        &input[offset..end],
                        child,
                        current_depth
                            .checked_add(1)
                            .ok_or(ProtocolCodecError::DepthExceeded)?,
                        maximum_depth,
                    )?;
                }
                offset = end;
            }
            5 => {
                let end = advance_end(input, offset, 4)?;
                if input[offset..end].iter().all(|byte| *byte == 0) {
                    return Err(ProtocolCodecError::NonCanonicalEncoding);
                }
                offset = end;
            }
            _ => return Err(ProtocolCodecError::WrongWireType),
        }
    }
    Ok(())
}

fn read_varint(input: &[u8], offset: &mut usize) -> Result<u64, ProtocolCodecError> {
    let start = *offset;
    let mut value = 0_u64;
    for index in 0_u32..10 {
        let byte = *input.get(*offset).ok_or(ProtocolCodecError::Truncated)?;
        *offset += 1;
        if index == 9 && byte > 1 {
            return Err(ProtocolCodecError::NonCanonicalVarint);
        }
        value |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            let width = *offset - start;
            if width > 1 && value < (1_u64 << ((width - 1) * 7)) {
                return Err(ProtocolCodecError::NonCanonicalVarint);
            }
            return Ok(value);
        }
    }
    Err(ProtocolCodecError::NonCanonicalVarint)
}

fn advance_end(input: &[u8], offset: usize, width: usize) -> Result<usize, ProtocolCodecError> {
    offset
        .checked_add(width)
        .filter(|end| *end <= input.len())
        .ok_or(ProtocolCodecError::Truncated)
}
