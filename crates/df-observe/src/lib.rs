//! Shared experimental OTEL conventions. Durable G06 export is not implemented.
use opentelemetry::trace::{Span, SpanContext, SpanId, TraceFlags, TraceId, TraceState, Tracer};
use opentelemetry::{Context, KeyValue, global};

/// Correlation is diagnostic provenance, never authorization.
#[derive(Clone)]
pub struct OperationContext {
    pub trace_parent: String,
    pub build: String,
}

fn parent(context: &OperationContext) -> Context {
    use opentelemetry::trace::TraceContextExt;
    let parts: Vec<_> = context.trace_parent.split('-').collect();
    if let ["00", trace, span, flags] = parts.as_slice()
        && let (Ok(trace), Ok(span), Ok(flags)) = (
            TraceId::from_hex(trace),
            SpanId::from_hex(span),
            u8::from_str_radix(flags, 16),
        )
    {
        return Context::new().with_remote_span_context(SpanContext::new(
            trace,
            span,
            TraceFlags::new(flags),
            true,
            TraceState::default(),
        ));
    }
    Context::new()
}
/// Owned OTEL operation span. Drop records abandonment unless its owner finishes it.
pub struct OperationSpan {
    span: global::BoxedSpan,
    finished: bool,
}
/// Begin an operation using explicit correlation across asynchronous dispatch.
pub fn begin(context: &OperationContext, method: &'static str) -> OperationSpan {
    let mut span =
        global::tracer("df-observe.transport-fixture").start_with_context(method, &parent(context));
    span.set_attribute(KeyValue::new("fixture.build", context.build.clone()));
    OperationSpan {
        span,
        finished: false,
    }
}
impl OperationSpan {
    /// Finish exactly once with a safe classification and actual observed byte count.
    pub fn finish(&mut self, status: &str, bytes: usize) {
        self.end(status, Some(bytes));
    }
    /// Finish without inventing a byte count when measurement is unavailable.
    pub fn finish_unmeasured(&mut self, status: &str) {
        self.end(status, None);
    }
    fn end(&mut self, status: &str, bytes: Option<usize>) {
        if self.finished {
            return;
        }
        self.span
            .set_attribute(KeyValue::new("rpc.status", status.to_owned()));
        self.span
            .set_attribute(KeyValue::new("rpc.bytes.measured", bytes.is_some()));
        if let Some(bytes) = bytes {
            self.span
                .set_attribute(KeyValue::new("rpc.bytes", bytes as i64));
        }
        self.span.end();
        self.finished = true;
    }
}

impl Drop for OperationSpan {
    fn drop(&mut self) {
        self.finish_unmeasured("abandoned");
    }
}
/// Record a bounded lifecycle event without recording payload bytes.
/// Browser sends trace context in generated RPC metadata; this fixture installs
/// an exporting SDK on the native side only.
pub fn record(context: &OperationContext, method: &'static str, status: &str, bytes: usize) {
    begin(context, method).finish(status, bytes);
}
#[cfg(not(target_arch = "wasm32"))]
pub use native::FixtureTelemetry;
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use opentelemetry::global;
    use opentelemetry_sdk::{
        error::{OTelSdkError, OTelSdkResult},
        trace::{SdkTracerProvider, SpanData, SpanExporter},
    };
    use std::{
        collections::VecDeque,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };
    const MAX_SPANS: usize = 512;
    #[derive(Clone, Debug, Default)]
    struct BoundedExporter {
        spans: Arc<Mutex<VecDeque<SpanData>>>,
        dropped: Arc<AtomicUsize>,
    }
    impl SpanExporter for BoundedExporter {
        async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
            let mut spans = self.spans.lock().map_err(|_| {
                OTelSdkError::InternalFailure("fixture export lock poisoned".to_owned())
            })?;
            for span in batch {
                if spans.len() == MAX_SPANS {
                    spans.pop_front();
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                }
                spans.push_back(span);
            }
            Ok(())
        }
    }
    /// Fixture-only bounded in-memory export. Not durable telemetry storage.
    pub struct FixtureTelemetry {
        provider: SdkTracerProvider,
        exporter: BoundedExporter,
    }
    impl Default for FixtureTelemetry {
        fn default() -> Self {
            let exporter = BoundedExporter::default();
            let provider = SdkTracerProvider::builder()
                .with_simple_exporter(exporter.clone())
                .build();
            global::set_tracer_provider(provider.clone());
            Self { provider, exporter }
        }
    }
    impl FixtureTelemetry {
        /// Return safe emitted span count; exporter failures remain visible.
        pub fn count(&self) -> Result<usize, String> {
            self.exporter
                .spans
                .lock()
                .map(|spans| spans.len())
                .map_err(|error| error.to_string())
        }
        /// Old diagnostic spans are discarded only at this declared fixture bound.
        pub fn dropped(&self) -> usize {
            self.exporter.dropped.load(Ordering::Relaxed)
        }
        /// Read-only safe OTEL records retain parent/trace IDs and typed attributes.
        pub fn snapshot(&self) -> Result<String, String> {
            let spans = self
                .exporter
                .spans
                .lock()
                .map_err(|error| error.to_string())?;
            Ok(spans
                .iter()
                .map(|span| {
                    format!(
                        "{} trace={} span={} parent={} duration_us={:?} attributes={:?}\n",
                        span.name,
                        span.span_context.trace_id(),
                        span.span_context.span_id(),
                        span.parent_span_id,
                        span.end_time
                            .duration_since(span.start_time)
                            .map(|duration| duration.as_micros()),
                        span.attributes
                    )
                })
                .collect())
        }
        /// Flush the actual SDK before preview shutdown.
        pub fn shutdown(&self) -> Result<(), String> {
            self.provider.shutdown().map_err(|error| error.to_string())
        }
    }
}
