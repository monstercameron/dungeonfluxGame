//! Native local telemetry contracts. Supplied diagnostic IDs grant no authority.
use std::{fmt, sync::mpsc, time::Duration};

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

/// Finite qualification limits. Smaller boundary fixtures may select smaller limits.
#[derive(Clone, Copy, Debug)]
pub struct TelemetryLimits {
    pub record_bytes: usize,
    pub batch_bytes: usize,
    pub batch_records: usize,
    pub queue_items: usize,
    pub queue_bytes: usize,
    pub spool_bytes: u64,
    pub sqlite_bytes: u64,
}
impl Default for TelemetryLimits {
    fn default() -> Self {
        Self {
            record_bytes: 8192,
            batch_bytes: 65536,
            batch_records: 32,
            queue_items: 1024,
            queue_bytes: 1048576,
            spool_bytes: 33554432,
            sqlite_bytes: 67108864,
        }
    }
}
impl TelemetryLimits {
    pub fn validate(self) -> Result<Self, TelemetryError> {
        let ceiling = Self::default();
        if self.record_bytes == 0
            || self.record_bytes > ceiling.record_bytes
            || self.batch_bytes == 0
            || self.batch_bytes > ceiling.batch_bytes
            || self.batch_records == 0
            || self.batch_records > ceiling.batch_records
            || self.queue_items == 0
            || self.queue_items > ceiling.queue_items
            || self.queue_bytes == 0
            || self.queue_bytes > ceiling.queue_bytes
            || self.spool_bytes == 0
            || self.spool_bytes > ceiling.spool_bytes
            || self.sqlite_bytes < 262144
            || self.sqlite_bytes > ceiling.sqlite_bytes
        {
            return Err(TelemetryError::InvalidLimits);
        }
        Ok(self)
    }
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

/// Merely enqueued work is unconfirmed and may be lost on process death.
#[derive(Debug)]
pub struct PendingReceipt(mpsc::Receiver<Result<IngestReceipt, TelemetryError>>);
impl PendingReceipt {
    pub fn channel() -> (mpsc::Sender<Result<IngestReceipt, TelemetryError>>, Self) {
        let (sender, receiver) = mpsc::channel();
        (sender, Self(receiver))
    }
    /// Poll without blocking SDK emission or input pacing. A returned receipt is final
    /// for this wait; discard its handle after observing it.
    pub fn try_receive(&self) -> Result<Option<IngestReceipt>, TelemetryError> {
        match self.0.try_recv() {
            Ok(result) => result.map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(TelemetryError::Closed),
        }
    }
    /// Deadline cancels only the wait; durably accepted owned work remains owned by ingestion.
    pub fn wait(self, timeout: Duration) -> Result<IngestReceipt, TelemetryError> {
        self.0.recv_timeout(timeout).map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => TelemetryError::Deadline,
            mpsc::RecvTimeoutError::Disconnected => TelemetryError::Closed,
        })?
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Durability {
    Spooled,
    Committed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IngestReceipt {
    pub durability: Durability,
    pub spool_position: u64,
    pub committed_watermark: u64,
    pub records: usize,
}
/// Object-safe native local port; implementations preflight bytes and fail fast on admission.
/// No filesystem or SQLite work is permitted in the calling producer's submission path.
pub trait TelemetryIngress: Send + Sync + fmt::Debug {
    fn submit(&self, signal: Signal, bytes: &[u8]) -> Result<PendingReceipt, TelemetryError>;
}
