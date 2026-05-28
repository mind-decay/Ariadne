//! Wire codec for the daemon protocol: length-prefixed postcard frames.
//!
//! Each exchange is one frame in each direction — a 4-byte big-endian
//! payload length followed by the postcard encoding of a [`DaemonRequest`]
//! or [`DaemonResponse`]. This module is the only place the postcard codec
//! appears, keeping the codec choice out of the domain interior
//! [src: docs/adr/0015-daemon-mode-ipc.md].

use std::io::{Read, Write};

use ariadne_core::{DaemonRequest, DaemonResponse};

use crate::errors::DaemonError;

/// Upper bound on an accepted frame payload, guarding a malformed length
/// prefix from demanding a huge allocation. Warm responses (markdown docs,
/// large symbol lists) sit far below this 64 MiB ceiling.
const MAX_FRAME: usize = 64 * 1024 * 1024;

/// Write a length-prefixed frame and flush it.
pub(crate) fn write_frame<W: Write>(w: &mut W, payload: &[u8]) -> Result<(), DaemonError> {
    let len = u32::try_from(payload.len())
        .map_err(|_| DaemonError::Protocol("frame payload too large".to_owned()))?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    w.flush()?;
    Ok(())
}

/// Read one length-prefixed frame, rejecting an oversized length prefix.
pub(crate) fn read_frame<R: Read>(r: &mut R) -> Result<Vec<u8>, DaemonError> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = usize::try_from(u32::from_be_bytes(len_buf))
        .map_err(|_| DaemonError::Protocol("frame length overflow".to_owned()))?;
    if len > MAX_FRAME {
        return Err(DaemonError::Protocol(format!(
            "frame length {len} exceeds cap {MAX_FRAME}"
        )));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(payload)
}

/// Encode a request to its postcard payload.
pub(crate) fn encode_request(req: &DaemonRequest) -> Result<Vec<u8>, DaemonError> {
    postcard::to_stdvec(req).map_err(|e| DaemonError::Protocol(e.to_string()))
}

/// Decode a request payload.
pub(crate) fn decode_request(payload: &[u8]) -> Result<DaemonRequest, DaemonError> {
    postcard::from_bytes(payload).map_err(|e| DaemonError::Protocol(e.to_string()))
}

/// Encode a response to its postcard payload.
pub(crate) fn encode_response(resp: &DaemonResponse) -> Result<Vec<u8>, DaemonError> {
    postcard::to_stdvec(resp).map_err(|e| DaemonError::Protocol(e.to_string()))
}

/// Decode a response payload.
pub(crate) fn decode_response(payload: &[u8]) -> Result<DaemonResponse, DaemonError> {
    postcard::from_bytes(payload).map_err(|e| DaemonError::Protocol(e.to_string()))
}
