//! Framing, sans-IO: a 4-byte big-endian length, then the postcard bytes.

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// No single frame is larger than this; a peer that says otherwise is broken.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum FrameError {
    TooLarge(usize),
    Postcard(postcard::Error),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::TooLarge(n) => write!(f, "frame of {n} bytes is over the {MAX_FRAME}-byte limit"),
            FrameError::Postcard(e) => write!(f, "postcard: {e}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// One value as one frame.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, FrameError> {
    let mut out = postcard::to_extend(value, vec![0u8; 4]).map_err(FrameError::Postcard)?;
    let len = out.len() - 4;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    out[..4].copy_from_slice(&(len as u32).to_be_bytes());
    Ok(out)
}

/// Cuts frames out of a stream's chunks, however the chunks fall.
#[derive(Debug, Default)]
pub struct Reader {
    buf: Vec<u8>,
}

impl Reader {
    pub fn push(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    /// The next whole frame, or `None` until more bytes arrive.
    pub fn next_frame<T: DeserializeOwned>(&mut self) -> Result<Option<T>, FrameError> {
        let Some(head) = self.buf.first_chunk::<4>() else {
            return Ok(None);
        };
        let len = u32::from_be_bytes(*head) as usize;
        if len > MAX_FRAME {
            return Err(FrameError::TooLarge(len));
        }
        if self.buf.len() < 4 + len {
            return Ok(None);
        }
        let value = postcard::from_bytes(&self.buf[4..4 + len]).map_err(FrameError::Postcard)?;
        self.buf.drain(..4 + len);
        Ok(Some(value))
    }

    /// Bytes received that are not yet a whole frame.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }
}
