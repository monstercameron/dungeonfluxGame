//! Owned native HTTPS execution. No redirects, proxies, resends, or detached tasks.

use std::{future::Future, time::Duration};

use crate::{HttpMethod, HttpRequest, ProviderFailureClass, ProviderRequestId};

/// Explicit positive transport bounds. Connect/read/total bounds cannot exceed
/// 180 seconds; response retention is at most 16 MiB and headers at most 64 KiB.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHttpLimits {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub total_timeout: Duration,
    pub maximum_request_bytes: usize,
    pub maximum_response_bytes: usize,
    pub maximum_header_bytes: usize,
    pub maximum_chunks: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHttpRefusal {
    InvalidLimits,
    InvalidEndpoint,
    InvalidCredential,
    RequestTooLarge,
    ResponseTooLarge,
    HeadersTooLarge,
    TooManyChunks,
    ContentType,
    Redirect,
    Cancelled,
    Deadline,
    Unknown,
}

impl NativeHttpRefusal {
    pub const fn failure_class(self) -> ProviderFailureClass {
        match self {
            Self::Cancelled => ProviderFailureClass::Cancelled,
            Self::Deadline => ProviderFailureClass::Deadline,
            Self::Unknown => ProviderFailureClass::Unknown,
            Self::InvalidLimits
            | Self::InvalidEndpoint
            | Self::InvalidCredential
            | Self::RequestTooLarge => ProviderFailureClass::InvalidRequest,
            _ => ProviderFailureClass::Contract,
        }
    }
}

pub(crate) struct NativeHttp {
    client: reqwest::Client,
    limits: NativeHttpLimits,
}

pub(crate) struct NativeHttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub transport_request_id: Option<ProviderRequestId>,
}

pub(crate) struct NativeHttpFailure {
    pub reason: NativeHttpRefusal,
    pub transport_request_id: Option<ProviderRequestId>,
}

impl NativeHttp {
    pub(crate) fn limits(&self) -> NativeHttpLimits {
        self.limits
    }
    pub(crate) fn new(
        limits: NativeHttpLimits,
        loopback_fixture: bool,
    ) -> Result<Self, NativeHttpRefusal> {
        let ceiling = Duration::from_secs(180);
        if [
            limits.connect_timeout,
            limits.read_timeout,
            limits.total_timeout,
        ]
        .into_iter()
        .any(|value| value.is_zero() || value > ceiling)
            || limits.maximum_request_bytes == 0
            || limits.maximum_request_bytes > 16_777_216
            || limits.maximum_response_bytes == 0
            || limits.maximum_response_bytes > 16_777_216
            || limits.maximum_header_bytes == 0
            || limits.maximum_header_bytes > 65_536
            || limits.maximum_chunks == 0
            || limits.maximum_chunks > 65_536
        {
            return Err(NativeHttpRefusal::InvalidLimits);
        }
        let client = reqwest::Client::builder()
            .tls_backend_rustls()
            .https_only(!loopback_fixture)
            .retry(reqwest::retry::never())
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .connection_verbose(false)
            .no_proxy()
            .connect_timeout(limits.connect_timeout)
            .read_timeout(limits.read_timeout)
            .timeout(limits.total_timeout)
            .build()
            .map_err(|_| NativeHttpRefusal::Unknown)?;
        Ok(Self { client, limits })
    }

    pub(crate) async fn execute(
        &self,
        request: HttpRequest,
        remaining_deadline: Duration,
        cancellation: impl Future<Output = ()>,
    ) -> Result<NativeHttpResponse, NativeHttpFailure> {
        let mut captured_id = None;
        let result = self
            .execute_owned(request, remaining_deadline, cancellation, &mut captured_id)
            .await;
        result.map_err(|reason| NativeHttpFailure {
            reason,
            transport_request_id: captured_id,
        })
    }

    async fn execute_owned(
        &self,
        request: HttpRequest,
        remaining_deadline: Duration,
        cancellation: impl Future<Output = ()>,
        captured_id: &mut Option<ProviderRequestId>,
    ) -> Result<NativeHttpResponse, NativeHttpRefusal> {
        if request.body().len() > self.limits.maximum_request_bytes {
            return Err(NativeHttpRefusal::RequestTooLarge);
        }
        let duration = remaining_deadline.min(self.limits.total_timeout);
        if duration.is_zero() {
            return Err(NativeHttpRefusal::Deadline);
        }
        // This single future owns send and body. Dropping it on cancellation,
        // deadline or caller drop releases the response and its network work.
        tokio::select! {
            biased;
            () = cancellation => Err(NativeHttpRefusal::Cancelled),
            result = tokio::time::timeout(duration, self.send_and_read(request, captured_id)) => {
                result.map_err(|_| NativeHttpRefusal::Deadline)?
            }
        }
    }

    async fn send_and_read(
        &self,
        request: HttpRequest,
        captured_id: &mut Option<ProviderRequestId>,
    ) -> Result<NativeHttpResponse, NativeHttpRefusal> {
        let method = match request.method() {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
        };
        let mut builder = self
            .client
            .request(method, request.url())
            .body(request.body().to_vec());
        for (name, value) in request.headers() {
            builder = builder.header(*name, value);
        }
        let mut response = builder.send().await.map_err(classify_transport_error)?;
        if let Some(value) = response.headers().get("x-request-id") {
            let value = value.to_str().map_err(|_| NativeHttpRefusal::Unknown)?;
            if value.len() > 256 {
                return Err(NativeHttpRefusal::HeadersTooLarge);
            }
            *captured_id =
                Some(ProviderRequestId::new(value).map_err(|_| NativeHttpRefusal::Unknown)?);
        }
        let status = response.status().as_u16();
        if response.status().is_redirection() {
            return Err(NativeHttpRefusal::Redirect);
        }
        let mut header_bytes = 0usize;
        for (name, value) in response.headers() {
            header_bytes = header_bytes
                .checked_add(name.as_str().len())
                .and_then(|count| count.checked_add(value.as_bytes().len()))
                .ok_or(NativeHttpRefusal::HeadersTooLarge)?;
        }
        if header_bytes > self.limits.maximum_header_bytes {
            return Err(NativeHttpRefusal::HeadersTooLarge);
        }
        let mut content_types = response
            .headers()
            .get_all(reqwest::header::CONTENT_TYPE)
            .iter();
        let content_type = content_types
            .next()
            .and_then(|value| value.to_str().ok())
            .ok_or(NativeHttpRefusal::ContentType)?;
        if content_types.next().is_some()
            || !content_type
                .split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
        {
            return Err(NativeHttpRefusal::ContentType);
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.limits.maximum_response_bytes as u64)
        {
            return Err(NativeHttpRefusal::ResponseTooLarge);
        }
        let mut body = Vec::new();
        let mut chunks = 0usize;
        while let Some(chunk) = response.chunk().await.map_err(classify_transport_error)? {
            chunks += 1;
            if chunks > self.limits.maximum_chunks {
                return Err(NativeHttpRefusal::TooManyChunks);
            }
            let length = body
                .len()
                .checked_add(chunk.len())
                .ok_or(NativeHttpRefusal::ResponseTooLarge)?;
            if length > self.limits.maximum_response_bytes {
                return Err(NativeHttpRefusal::ResponseTooLarge);
            }
            body.try_reserve(chunk.len())
                .map_err(|_| NativeHttpRefusal::ResponseTooLarge)?;
            body.extend_from_slice(&chunk);
        }
        // Only actual chunk() == None produces this complete observation.
        Ok(NativeHttpResponse {
            status,
            body,
            transport_request_id: captured_id.clone(),
        })
    }
}

fn classify_transport_error(error: reqwest::Error) -> NativeHttpRefusal {
    if error.is_timeout() {
        NativeHttpRefusal::Deadline
    } else {
        NativeHttpRefusal::Unknown
    }
}
