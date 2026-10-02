//! Shared capture identities; these values grant no authority.
use std::fmt;

/// Caller-owned source lifecycle identity; uniqueness across lifetimes is a caller duty.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProducerId([u8; 16]);
/// Stable record identity captured before SDK dispatch and retained for retries.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordId([u8; 16]);
macro_rules! identity {
    ($name:ident) => {
        impl $name {
            pub fn new(bytes: [u8; 16]) -> Result<Self, TelemetryError> {
                if bytes == [0; 16] {
                    return Err(TelemetryError::InvalidIdentity);
                }
                Ok(Self(bytes))
            }
            pub fn bytes(self) -> [u8; 16] {
                self.0
            }
            pub fn hex(self) -> String {
                self.0.iter().map(|byte| format!("{byte:02x}")).collect()
            }
            pub fn from_hex(text: &str) -> Result<Self, TelemetryError> {
                if text.len() != 32
                    || !text
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(TelemetryError::InvalidIdentity);
                }
                let mut bytes = [0; 16];
                for (out, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
                    let value =
                        std::str::from_utf8(pair).map_err(|_| TelemetryError::InvalidIdentity)?;
                    *out = u8::from_str_radix(value, 16)
                        .map_err(|_| TelemetryError::InvalidIdentity)?;
                }
                Self::new(bytes)
            }
        }
    };
}
identity!(ProducerId);
identity!(RecordId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSequence(i64);
impl SourceSequence {
    pub fn new(value: i64) -> Result<Self, TelemetryError> {
        if value <= 0 {
            return Err(TelemetryError::InvalidIdentity);
        }
        Ok(Self(value))
    }
    pub fn get(self) -> i64 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureKey {
    pub producer: ProducerId,
    pub record: RecordId,
    pub sequence: SourceSequence,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    Logs,
    Spans,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TelemetryError {
    InvalidIdentity,
    SequenceExhausted,
    InvalidLimits,
    Malformed,
    Oversized,
    Capacity,
    Closed,
    Deadline,
    Conflict,
    Corrupt,
    ForeignRoot,
    Ownership,
    Io,
    SinkUnavailable,
    Internal,
}
impl fmt::Display for TelemetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for TelemetryError {}
