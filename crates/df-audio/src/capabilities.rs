//! Pure capture codec selection from adapter-reported formats.
//!
//! These reports drive an exact lease-format match only. They do not probe or
//! qualify a browser, device, microphone, permission grant, or delivered audio.

use df_types::RevisionLabel;

use crate::capture::CaptureError;

pub(super) fn select_capture_codec<'a>(
    lease_format: &'a RevisionLabel,
    adapter_formats: &[RevisionLabel],
) -> Result<&'a RevisionLabel, CaptureError> {
    if adapter_formats
        .iter()
        .any(|adapter_format| adapter_format == lease_format)
    {
        return Ok(lease_format);
    }

    Err(CaptureError::UnsupportedFormat)
}
