//! zlib-stream inflater: one shared inflater per connection, `00 00 FF FF` suffix framing, caps on
//! the compressed buffer and the decompressed message. Adapted from Serein's
//! `crates/discord-gateway/src/compression.rs` (MIT OR Apache-2.0). Takes the transport's raw
//! frames (plan NET).
use crate::transport::Frame;
use flate2::{Decompress, FlushDecompress, Status};

/// Cap for both the compressed buffer and one decompressed message. Large accounts' READY exceeds
/// common 16 MiB WebSocket defaults, so the transport's message-size limit is this same bound.
pub(crate) const MAX_GATEWAY_WIRE: usize = 64 << 20;
const RETAINED_PENDING_BYTES: usize = 128 * 1024;
const SUFFIX: [u8; 4] = [0, 0, 255, 255];

/// Fixed-label failure; never carries remote data.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DecodeError {
    Protocol,
    CompressedCap,
    DecompressedCap,
}

pub(crate) struct Decoder {
    inflater: Decompress,
    pending: Vec<u8>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            inflater: Decompress::new(true),
            pending: Vec::new(),
        }
    }
}

impl Decoder {
    /// The text of one whole message, or `Ok(None)` while a partial one is buffered. Text frames
    /// pass through unchanged.
    pub(crate) fn frame(&mut self, frame: Frame) -> Result<Option<String>, DecodeError> {
        let bytes = match frame {
            Frame::Binary(bytes) => bytes,
            Frame::Text(_) if !self.pending.is_empty() => return Err(DecodeError::Protocol),
            Frame::Text(mut text) => return Ok(Some(std::mem::take(&mut *text))),
        };
        if bytes.len() > MAX_GATEWAY_WIRE.saturating_sub(self.pending.len()) {
            return Err(DecodeError::CompressedCap);
        }
        self.pending.extend_from_slice(&bytes);
        if !self.pending.ends_with(&SUFFIX) {
            return Ok(None);
        }
        let mut output = Vec::new();
        let mut consumed = 0;
        loop {
            let mut chunk = [0; 16 * 1024];
            let before_in = self.inflater.total_in();
            let before_out = self.inflater.total_out();
            let status = self
                .inflater
                .decompress(&self.pending[consumed..], &mut chunk, FlushDecompress::Sync)
                .map_err(|_| DecodeError::Protocol)?;
            let read = (self.inflater.total_in() - before_in) as usize;
            let written = (self.inflater.total_out() - before_out) as usize;
            if status == Status::StreamEnd {
                return Err(DecodeError::Protocol);
            }
            if written > MAX_GATEWAY_WIRE.saturating_sub(output.len()) {
                return Err(DecodeError::DecompressedCap);
            }
            output.extend_from_slice(&chunk[..written]);
            consumed += read;
            if consumed == self.pending.len() && written < chunk.len() {
                break;
            }
            if read == 0 && written == 0 {
                return Err(DecodeError::Protocol);
            }
        }
        self.pending.clear();
        // Do not retain a large READY allocation for the connection's lifetime. The inflater's
        // dictionary stays intact.
        if self.pending.capacity() > RETAINED_PENDING_BYTES {
            self.pending = Vec::new();
        }
        String::from_utf8(output)
            .map(Some)
            .map_err(|_| DecodeError::Protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn text(s: &str) -> String {
        s.to_owned()
    }

    #[test]
    fn decoder_buffers_a_frame_split_across_the_suffix() {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(br#"{"op":11,"d":null}"#).unwrap();
        encoder.flush().unwrap();
        let bytes = encoder.get_ref().clone();
        assert!(bytes.ends_with(&SUFFIX));
        let split = bytes.len() - 2; // between the two halves of the suffix
        let mut decoder = Decoder::default();
        assert!(decoder
            .frame(Frame::Binary(bytes[..split].to_vec()))
            .unwrap()
            .is_none());
        assert_eq!(
            decoder
                .frame(Frame::Binary(bytes[split..].to_vec()))
                .unwrap(),
            Some(text(r#"{"op":11,"d":null}"#))
        );
    }

    #[test]
    fn decoder_shares_the_dictionary_across_messages() {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut decoder = Decoder::default();
        let mut offset = 0;
        let mut second = Vec::new();
        // The second message back-references the first, so it only inflates with the shared window.
        for msg in [
            r#"{"op":10,"d":{"heartbeat_interval":1000}}"#,
            r#"{"op":10,"d":{"heartbeat_interval":1000}}"#,
        ] {
            encoder.write_all(msg.as_bytes()).unwrap();
            encoder.flush().unwrap();
            let bytes = encoder.get_ref()[offset..].to_vec();
            offset = encoder.get_ref().len();
            second = bytes.clone();
            assert_eq!(
                decoder.frame(Frame::Binary(bytes)).unwrap(),
                Some(text(msg))
            );
        }
        // Without the shared window the second message alone is undecodable.
        assert!(Decoder::default().frame(Frame::Binary(second)).is_err());
    }

    #[test]
    fn decoder_rejects_oversized_compressed_input() {
        assert_eq!(
            Decoder::default().frame(Frame::Binary(vec![0; MAX_GATEWAY_WIRE + 1])),
            Err(DecodeError::CompressedCap)
        );
    }

    #[test]
    fn decoder_rejects_oversized_decompressed_output() {
        let mut bomb = flate2::Compress::new(flate2::Compression::fast(), true);
        let mut compressed = Vec::with_capacity(MAX_GATEWAY_WIRE + 1024);
        bomb.compress_vec(
            &vec![b'x'; MAX_GATEWAY_WIRE + 1],
            &mut compressed,
            flate2::FlushCompress::Sync,
        )
        .unwrap();
        assert!(compressed.ends_with(&SUFFIX));
        assert!(compressed.len() < MAX_GATEWAY_WIRE);
        assert_eq!(
            Decoder::default().frame(Frame::Binary(compressed)),
            Err(DecodeError::DecompressedCap)
        );
    }
}
