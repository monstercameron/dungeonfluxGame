use crate::{
    CaptureKey, PendingReceipt, ProducerId, RecordId, Signal, SourceSequence, TelemetryError,
    TelemetryIngress, TelemetryLimits,
};
use df_types::{BuildIdentity, BuildRevision};
use opentelemetry::{
    Context, InstrumentationScope, KeyValue,
    logs::{LogRecord, Logger, LoggerProvider},
    trace::{Span, Tracer, TracerProvider},
};
use opentelemetry_proto::{
    tonic::{
        collector::{logs::v1::ExportLogsServiceRequest, trace::v1::ExportTraceServiceRequest},
        logs::v1::ResourceLogs,
        trace::v1::ResourceSpans,
    },
    transform::common::tonic::ResourceAttributesWithSchema,
};
use opentelemetry_sdk::{
    Resource,
    error::{OTelSdkError, OTelSdkResult},
    logs::{LogBatch, LogExporter, SdkLoggerProvider},
    trace::{Sampler, SdkTracerProvider, SpanData, SpanExporter},
};
use prost::Message;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

pub use opentelemetry::logs::{AnyValue, Severity};
/// Safe synthetic/native capture input. The owner supplies explicit timestamps and correlation.
#[derive(Debug)]
pub struct LogInput {
    pub timestamp: SystemTime,
    pub observed_timestamp: SystemTime,
    pub severity: Severity,
    pub body: AnyValue,
    pub attributes: Vec<(String, AnyValue)>,
    pub trace: Option<opentelemetry::trace::SpanContext>,
}
#[derive(Debug)]
pub struct SpanInput {
    pub name: String,
    pub start: SystemTime,
    pub end: SystemTime,
    pub parent: Context,
    pub links: Vec<opentelemetry::trace::Link>,
    pub attributes: Vec<KeyValue>,
}
/// Retry exactly these standard OTLP bytes; this identity never changes on a lost receipt.
#[derive(Debug)]
pub struct CapturedBatch {
    pub signal: Signal,
    pub bytes: Vec<u8>,
    pub pending: PendingReceipt,
}
type Submission = Arc<Mutex<Option<Result<CapturedBatch, TelemetryError>>>>;
#[derive(Debug)]
struct NativeExporter {
    ingress: Arc<dyn TelemetryIngress>,
    resource: ResourceAttributesWithSchema,
    result: Submission,
    limits: TelemetryLimits,
}
impl NativeExporter {
    fn submit(&self, signal: Signal, bytes: Vec<u8>) -> OTelSdkResult {
        let result = if bytes.len() > self.limits.batch_bytes {
            Err(TelemetryError::Oversized)
        } else {
            self.ingress
                .submit(signal, &bytes)
                .map(|pending| CapturedBatch {
                    signal,
                    bytes,
                    pending,
                })
        };
        let failed = result.is_err();
        let mut slot = self
            .result
            .lock()
            .map_err(|_| OTelSdkError::InternalFailure("native capture slot poisoned".into()))?;
        *slot = Some(result);
        if failed {
            Err(OTelSdkError::InternalFailure(
                "native bounded admission refused".into(),
            ))
        } else {
            Ok(())
        }
    }
}
impl LogExporter for NativeExporter {
    async fn export(&self, batch: LogBatch<'_>) -> OTelSdkResult {
        // Single-resource conversion preserves instrumentation scope schema (the upstream
        // grouping helper instead uses resource schema for logs in SDK 0.31).
        let resource_logs = batch
            .iter()
            .map(|pair| ResourceLogs::from((pair, &self.resource)))
            .collect();
        self.submit(
            Signal::Logs,
            ExportLogsServiceRequest { resource_logs }.encode_to_vec(),
        )
    }
    fn set_resource(&mut self, resource: &Resource) {
        self.resource = ordered_resource(resource);
    }
}
impl SpanExporter for NativeExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let resource_spans = batch
            .into_iter()
            .map(|span| ResourceSpans::new(span, &self.resource))
            .collect();
        self.submit(
            Signal::Spans,
            ExportTraceServiceRequest { resource_spans }.encode_to_vec(),
        )
    }
    fn set_resource(&mut self, resource: &Resource) {
        self.resource = ordered_resource(resource);
    }
}
/// Owner of actual SDK providers and sequential native capture. SDK callbacks do bounded
/// encoding/queue admission only; the independent ingress worker owns all disk work.
pub struct NativeProducer {
    source: ProducerId,
    next: Option<i64>,
    logs: SdkLoggerProvider,
    traces: SdkTracerProvider,
    result: Submission,
    scope: InstrumentationScope,
    limits: TelemetryLimits,
}
impl NativeProducer {
    pub fn new(
        source: ProducerId,
        first_sequence: SourceSequence,
        build: &BuildIdentity,
        ingress: Arc<dyn TelemetryIngress>,
        limits: TelemetryLimits,
    ) -> Result<Self, TelemetryError> {
        let limits = limits.validate()?;
        let result = Arc::new(Mutex::new(None));
        let resource = Resource::builder_empty()
            .with_attributes([
                KeyValue::new("service.name", "df-telemetry.synthetic-native"),
                KeyValue::new(
                    "df.build",
                    build.revision(BuildRevision::Source).as_str().to_owned(),
                ),
                KeyValue::new(
                    "df.native",
                    build.revision(BuildRevision::Native).as_str().to_owned(),
                ),
                KeyValue::new(
                    "df.wasm",
                    build.revision(BuildRevision::Wasm).as_str().to_owned(),
                ),
                KeyValue::new(
                    "df.configuration",
                    build
                        .revision(BuildRevision::Configuration)
                        .as_str()
                        .to_owned(),
                ),
                KeyValue::new(
                    "df.content",
                    build.revision(BuildRevision::Content).as_str().to_owned(),
                ),
            ])
            .with_schema_url([], "https://dungeonflux.local/telemetry/native-v1")
            .build();
        let exporter = || NativeExporter {
            ingress: Arc::clone(&ingress),
            resource: ordered_resource(&resource),
            result: Arc::clone(&result),
            limits,
        };
        let logs = SdkLoggerProvider::builder()
            .with_resource(resource.clone())
            .with_simple_exporter(exporter())
            .build();
        let span_exporter = exporter();
        let traces = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .with_resource(resource)
            .with_simple_exporter(span_exporter)
            .build();
        let scope = InstrumentationScope::builder("df-observe.native")
            .with_version("0.1.0")
            .with_schema_url("https://dungeonflux.local/telemetry/scope-v1")
            .with_attributes([KeyValue::new("fixture.synthetic", true)])
            .build();
        Ok(Self {
            source,
            next: Some(first_sequence.get()),
            logs,
            traces,
            result,
            scope,
            limits,
        })
    }
    /// Reserve identity before dispatch. Failed capture still consumes sequence and creates a
    /// possible source gap. Caller never restarts a source lifecycle at an already used sequence.
    pub fn reserve(&mut self, record: RecordId) -> Result<CaptureKey, TelemetryError> {
        let sequence = self.next.ok_or(TelemetryError::SequenceExhausted)?;
        self.next = sequence.checked_add(1);
        Ok(CaptureKey {
            producer: self.source,
            record,
            sequence: SourceSequence::new(sequence)?,
        })
    }
    fn take_submission(&self) -> Result<CapturedBatch, TelemetryError> {
        self.result
            .lock()
            .map_err(|_| TelemetryError::Internal)?
            .take()
            .ok_or(TelemetryError::Internal)?
    }
    pub fn emit_log(
        &mut self,
        key: CaptureKey,
        input: LogInput,
    ) -> Result<CapturedBatch, TelemetryError> {
        if key.producer != self.source || input.attributes.len() > 64 {
            return Err(TelemetryError::InvalidIdentity);
        }
        let body_bytes = value_bytes(&input.body, 0)?;
        let attribute_bytes =
            input
                .attributes
                .iter()
                .try_fold(0usize, |bytes, (name, value)| {
                    if name.starts_with("df.producer_id")
                        || name == "df.record_id"
                        || name == "df.source_sequence"
                    {
                        return Err(TelemetryError::InvalidIdentity);
                    }
                    Ok(bytes
                        .saturating_add(name.len())
                        .saturating_add(value_bytes(value, 0)?))
                })?;
        if body_bytes.saturating_add(attribute_bytes) > self.limits.record_bytes {
            return Err(TelemetryError::Oversized);
        }
        validate_time(input.timestamp)?;
        validate_time(input.observed_timestamp)?;
        let logger = self.logs.logger_with_scope(self.scope.clone());
        let mut record = logger.create_log_record();
        record.set_timestamp(input.timestamp);
        record.set_observed_timestamp(input.observed_timestamp);
        record.set_severity_number(input.severity);
        record.set_severity_text("synthetic");
        record.set_event_name("native.capture");
        record.set_body(input.body);
        record.add_attributes(input.attributes);
        record.add_attribute("df.producer_id", key.producer.hex());
        record.add_attribute("df.record_id", key.record.hex());
        record.add_attribute("df.source_sequence", key.sequence.get());
        if let Some(trace) = input.trace {
            record.set_trace_context(trace.trace_id(), trace.span_id(), Some(trace.trace_flags()));
        }
        logger.emit(record);
        self.take_submission()
    }
    pub fn emit_span(
        &mut self,
        key: CaptureKey,
        mut input: SpanInput,
    ) -> Result<CapturedBatch, TelemetryError> {
        if key.producer != self.source
            || input.attributes.len() > 64
            || input.links.len() > 16
            || input.name.len() > 256
        {
            return Err(TelemetryError::Oversized);
        }
        if input.attributes.iter().any(|attribute| {
            matches!(
                attribute.key.as_str(),
                "df.producer_id" | "df.record_id" | "df.source_sequence"
            )
        }) {
            return Err(TelemetryError::InvalidIdentity);
        }
        let mut measured = input.name.len();
        for attribute in &input.attributes {
            measured = add_limited(
                measured,
                attribute.key.as_str().len() + span_value_bytes(&attribute.value)?,
            )?;
        }
        for link in &input.links {
            if link.attributes.len() > 64 {
                return Err(TelemetryError::Oversized);
            }
            measured = add_limited(measured, 64)?;
            for attribute in &link.attributes {
                measured = add_limited(
                    measured,
                    attribute.key.as_str().len() + span_value_bytes(&attribute.value)?,
                )?;
            }
        }
        if measured > self.limits.record_bytes {
            return Err(TelemetryError::Oversized);
        }
        validate_time(input.start)?;
        validate_time(input.end)?;
        if input.end < input.start {
            return Err(TelemetryError::Malformed);
        }
        input.attributes.extend([
            KeyValue::new("df.producer_id", key.producer.hex()),
            KeyValue::new("df.record_id", key.record.hex()),
            KeyValue::new("df.source_sequence", key.sequence.get()),
        ]);
        let tracer = self.traces.tracer_with_scope(self.scope.clone());
        let builder = tracer
            .span_builder(input.name)
            .with_start_time(input.start)
            .with_attributes(input.attributes)
            .with_links(input.links);
        let mut span = tracer.build_with_context(builder, &input.parent);
        span.end_with_timestamp(input.end);
        self.take_submission()
    }
    pub fn shutdown(self, timeout: Duration) -> Result<(), TelemetryError> {
        self.logs
            .shutdown_with_timeout(timeout)
            .map_err(|_| TelemetryError::Internal)?;
        self.traces
            .shutdown_with_timeout(timeout)
            .map_err(|_| TelemetryError::Internal)
    }
}
fn validate_time(time: SystemTime) -> Result<(), TelemetryError> {
    let nanos = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| TelemetryError::Malformed)?
        .as_nanos();
    if nanos > u128::from(u64::MAX) {
        return Err(TelemetryError::Oversized);
    }
    Ok(())
}
fn value_bytes(value: &AnyValue, depth: usize) -> Result<usize, TelemetryError> {
    if depth > 8 {
        return Err(TelemetryError::Oversized);
    }
    match value {
        AnyValue::String(value) => add_limited(0, value.as_str().len()),
        AnyValue::Bytes(value) => add_limited(0, value.len()),
        AnyValue::Int(_) | AnyValue::Double(_) | AnyValue::Boolean(_) => Ok(8),
        AnyValue::ListAny(values) => {
            if values.len() > 64 {
                return Err(TelemetryError::Oversized);
            }
            values.iter().try_fold(0usize, |bytes, value| {
                add_limited(bytes, value_bytes(value, depth + 1)?)
            })
        }
        AnyValue::Map(values) => {
            if values.len() > 64 {
                return Err(TelemetryError::Oversized);
            }
            values.iter().try_fold(0usize, |bytes, (name, value)| {
                add_limited(
                    bytes,
                    name.as_str()
                        .len()
                        .saturating_add(value_bytes(value, depth + 1)?),
                )
            })
        }
        _ => Err(TelemetryError::Malformed),
    }
}

fn add_limited(left: usize, right: usize) -> Result<usize, TelemetryError> {
    left.checked_add(right)
        .filter(|bytes| *bytes <= 8192)
        .ok_or(TelemetryError::Oversized)
}
fn span_value_bytes(value: &opentelemetry::Value) -> Result<usize, TelemetryError> {
    use opentelemetry::{Array, Value};
    match value {
        Value::Bool(_) | Value::I64(_) | Value::F64(_) => Ok(8),
        Value::String(value) => add_limited(0, value.as_str().len()),
        Value::Array(array) => match array {
            Array::Bool(values) => {
                if values.len() <= 64 {
                    Ok(values.len())
                } else {
                    Err(TelemetryError::Oversized)
                }
            }
            Array::I64(values) => {
                if values.len() <= 64 {
                    Ok(values.len() * 8)
                } else {
                    Err(TelemetryError::Oversized)
                }
            }
            Array::F64(values) => {
                if values.len() <= 64 {
                    Ok(values.len() * 8)
                } else {
                    Err(TelemetryError::Oversized)
                }
            }
            Array::String(values) => {
                if values.len() > 64 {
                    return Err(TelemetryError::Oversized);
                }
                values.iter().try_fold(0usize, |bytes, value| {
                    add_limited(bytes, value.as_str().len())
                })
            }
            _ => Err(TelemetryError::Malformed),
        },
        _ => Err(TelemetryError::Malformed),
    }
}

fn ordered_resource(resource: &Resource) -> ResourceAttributesWithSchema {
    let mut converted: ResourceAttributesWithSchema = resource.into();
    // SDK Resource uses a hash map. Stable encoding prevents fresh SDK construction
    // from changing bytes solely because hash iteration order differs after restart.
    converted
        .attributes
        .0
        .sort_by(|left, right| left.key.cmp(&right.key));
    converted
}
