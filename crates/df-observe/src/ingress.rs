//! Native local telemetry contracts. Supplied diagnostic IDs grant no authority.
use std::{fmt, sync::mpsc, time::Duration};

use crate::{Signal, TelemetryError};

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
