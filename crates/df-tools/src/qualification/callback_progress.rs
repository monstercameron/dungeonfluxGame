//! Bounded observations of real rAF and connection counters, not a browser simulator.
use std::fmt::{self, Write};

pub(crate) const MAX_FRAME_SAMPLES: usize = 128;
const TIMELINE_INTERVAL_MS: f64 = 250.0;
const ALL_CLASSES: u8 = 0b1111;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FramePoint {
    pub time_ms: f64,
    pub callback_bytes: usize,
    pub callback_items: usize,
    pub decode_yields: usize,
    pub completed_classes: u8,
}
#[derive(Debug)]
struct TimelinePoint {
    frame: FramePoint,
    preceding_gap_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObservationError {
    InvalidTimestamp,
    InvalidClasses,
    CounterRegression,
    CompletionRegression,
    SampleLimit,
    NoOverlap,
}
impl fmt::Display for ObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTimestamp => "animation timestamps are not finite, nonnegative and increasing",
            Self::InvalidClasses => "completed workload mask contains an unknown class",
            Self::CounterRegression => "connection callback or decode counters regressed",
            Self::CompletionRegression => "a completed workload class became active again",
            Self::SampleLimit => "animation observation reached its retained sample limit",
            Self::NoOverlap => "no interval observed callback and decode progress between frames during active pressure",
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct CallbackProgress {
    points: Vec<TimelinePoint>,
    previous: Option<FramePoint>,
    witness: Option<(FramePoint, FramePoint)>,
    overlap_intervals: usize,
    max_adjacent_gap_ms: Option<f64>,
    seen: usize,
    rejected: Option<FramePoint>,
    error: Option<ObservationError>,
}
impl CallbackProgress {
    pub(crate) fn record(&mut self, point: FramePoint) -> Result<(), ObservationError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.seen += 1;
        let error = if !point.time_ms.is_finite() || point.time_ms < 0.0 {
            Some(ObservationError::InvalidTimestamp)
        } else if point.completed_classes & !ALL_CLASSES != 0 {
            Some(ObservationError::InvalidClasses)
        } else if let Some(previous) = self.previous {
            if point.time_ms <= previous.time_ms {
                Some(ObservationError::InvalidTimestamp)
            } else if point.callback_bytes < previous.callback_bytes
                || point.callback_items < previous.callback_items
                || point.decode_yields < previous.decode_yields
            {
                Some(ObservationError::CounterRegression)
            } else if point.completed_classes & previous.completed_classes
                != previous.completed_classes
            {
                Some(ObservationError::CompletionRegression)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(error) = error {
            return self.reject(point, error);
        }
        let gap = self
            .previous
            .map(|previous| point.time_ms - previous.time_ms);
        if let Some(previous) = self.previous {
            let adjacent_gap = point.time_ms - previous.time_ms;
            self.max_adjacent_gap_ms = Some(
                self.max_adjacent_gap_ms
                    .map_or(adjacent_gap, |maximum| maximum.max(adjacent_gap)),
            );
            if previous.completed_classes != ALL_CLASSES
                && point.completed_classes != ALL_CLASSES
                && point.callback_bytes > previous.callback_bytes
                && point.callback_items > previous.callback_items
                && point.decode_yields > previous.decode_yields
            {
                self.overlap_intervals += 1;
                self.witness.get_or_insert((previous, point));
            }
        }
        // Validate every delivered frame, but retain a periodic timeline plus completion.
        // At most122 timeline entries fit the owned30s run, regardless of refresh rate.
        // The exact adjacent witness is independent of this sampling cadence.
        let retain = self
            .points
            .last()
            .is_none_or(|last| point.time_ms - last.frame.time_ms >= TIMELINE_INTERVAL_MS)
            || (point.completed_classes == ALL_CLASSES
                && self
                    .previous
                    .is_some_and(|previous| previous.completed_classes != ALL_CLASSES));
        if retain {
            if self.points.len() == MAX_FRAME_SAMPLES {
                return self.reject(point, ObservationError::SampleLimit);
            }
            self.points.push(TimelinePoint {
                frame: point,
                preceding_gap_ms: gap,
            });
        }
        self.previous = Some(point);
        Ok(())
    }

    fn reject(
        &mut self,
        point: FramePoint,
        error: ObservationError,
    ) -> Result<(), ObservationError> {
        self.rejected = Some(point);
        self.error = Some(error);
        Err(error)
    }

    /// Only an exact validated adjacent pair proves local overlap, never timeline deltas.
    /// Invalid or truncated observations cannot be promoted by an earlier good witness.
    pub(crate) fn qualify(&self) -> Result<(), ObservationError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.witness.map(|_| ()).ok_or(ObservationError::NoOverlap)
    }

    /// Actual immediately preceding rAF gaps at retained points, not timeline spacing.
    pub(crate) fn gaps(&self) -> Vec<f64> {
        self.points
            .iter()
            .filter_map(|point| point.preceding_gap_ms)
            .collect()
    }

    pub(crate) fn report(&self) -> String {
        let mut report = format!(
            "Callback/render overlap observation: {:?}; delivered={} retained={} limit={} timeline_unretained={} truncated={} overlap_intervals={}\nTimeline cadence={}ms plus first/completion; every delivered frame validates timestamp/counters/classes. Exact adjacent witness is retained separately. Timeline spacing is not animation latency; sampled gap percentiles describe only retained preceding gaps.\nMaximum adjacent rAF gap across validated frames: {:?}ms\nSampled preceding adjacent rAF gaps ms: {:?}\nScope: witnessed frame/callback/decode overlap only; no production latency budget or browser engine memory claim. Native verdict tests do not prove browser behavior.\nTimeline (rAF ms, callback bytes total, callback items total, decode yields, completed class mask, actual preceding gap ms):\n",
            self.qualify(),
            self.seen,
            self.points.len(),
            MAX_FRAME_SAMPLES,
            self.seen.saturating_sub(self.points.len()),
            self.error == Some(ObservationError::SampleLimit),
            self.overlap_intervals,
            TIMELINE_INTERVAL_MS,
            self.max_adjacent_gap_ms,
            self.gaps(),
        );
        for sample in &self.points {
            let point = sample.frame;
            let _ = writeln!(
                report,
                "{:?},{},{},{},{:04b},{:?}",
                point.time_ms,
                point.callback_bytes,
                point.callback_items,
                point.decode_yields,
                point.completed_classes,
                sample.preceding_gap_ms
            );
        }
        if let Some((previous, current)) = self.witness {
            let _ = writeln!(
                report,
                "Exact adjacent overlap witness: {previous:?} -> {current:?}; gap_ms={:?}",
                current.time_ms - previous.time_ms
            );
        }
        if let Some(point) = self.rejected {
            let _ = writeln!(report, "Rejected observation: {point:?}");
        }
        report
    }
}

/// Observer-free measured text remains owned outside the deadline-cancelled run.
pub(crate) struct PressureReport {
    pub text: String,
    pub retained_in_failure: bool,
}

/// Preserve measured snapshots/build/cleanup without parsing diagnostic strings.
pub(crate) fn terminal_report(
    mut terminal: String,
    measured: Option<&PressureReport>,
    observations: &CallbackProgress,
) -> String {
    if let Some(measured) = measured
        && !measured.retained_in_failure
    {
        terminal.push_str("\nLast measured pressure report:\n");
        terminal.push_str(&measured.text);
    }
    terminal.push('\n');
    terminal.push_str(&observations.report());
    terminal
}
