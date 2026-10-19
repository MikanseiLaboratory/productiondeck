//! Chunked image assembly.
//!
//! The Gen2 image commands split a JPEG across output reports. The assembler
//! tracks sequence numbers and the "transfer is done" flag and hands the
//! completed JPEG back to the caller.

use crate::model::MAX_PANELS;
use heapless::Vec;

/// What is currently being assembled.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Key,
    FullScreen,
    Panel,
    Window,
    Background,
    BootLogo,
}

/// Chunk metadata shared by all image commands.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkInfo {
    pub kind: ImageKind,
    /// Key index (`ImageKind::Key`), background index (`Background`) or panel index.
    pub slot: u8,
    /// Region for partial-window transfers.
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub sequence: u16,
    pub is_last: bool,
}

/// Result of feeding one chunk into the assembler.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingest {
    /// Chunk accepted, more chunks are expected.
    InProgress,
    /// The image is complete; read [`ImageAssembler::bytes`].
    Complete,
    /// The chunk did not belong to the active transfer and was ignored.
    Ignored,
}

/// Errors produced while assembling an image.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// The chunk declared more bytes than the fixed buffer can hold.
    BufferFull,
    /// Sequence numbers went backwards or skipped.
    BadSequence,
    /// A chunk was shorter than its declared payload.
    ShortChunk,
}

/// Fixed-capacity image assembler.
#[derive(Debug)]
pub struct ImageAssembler<const N: usize> {
    buf: Vec<u8, N>,
    active: Option<ChunkInfo>,
    expected_sequence: u16,
}

impl<const N: usize> Default for ImageAssembler<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> ImageAssembler<N> {
    pub const fn new() -> Self {
        Self {
            buf: Vec::new(),
            active: None,
            expected_sequence: 0,
        }
    }

    /// Drop any in-flight transfer.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.active = None;
        self.expected_sequence = 0;
    }

    /// Completed image bytes (valid after [`Ingest::Complete`]).
    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Metadata of the active or last completed transfer.
    pub fn info(&self) -> Option<ChunkInfo> {
        self.active
    }

    /// True if a transfer is in progress.
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Feed one chunk.
    ///
    /// `sequence == 0` starts (or restarts) a transfer. Chunks with a
    /// mismatching sequence reset the assembler and return
    /// [`ImageError::BadSequence`].
    pub fn ingest(&mut self, info: ChunkInfo, data: &[u8]) -> Result<Ingest, ImageError> {
        if info.sequence == 0 {
            self.buf.clear();
            self.active = Some(info);
            self.expected_sequence = 0;
        }

        let active = match self.active {
            Some(a) if a.kind == info.kind && a.slot == info.slot => a,
            _ => {
                self.reset();
                return Ok(Ingest::Ignored);
            }
        };

        // Partial-window transfers must keep the same rectangle.
        if active.kind == ImageKind::Panel
            && active.sequence != 0
            && (active.x, active.y, active.w, active.h) != (info.x, info.y, info.w, info.h)
        {
            self.reset();
            return Ok(Ingest::Ignored);
        }

        if info.sequence != self.expected_sequence {
            self.reset();
            return Err(ImageError::BadSequence);
        }

        if info.is_last {
            // Keep the recorded geometry for the final chunk.
            self.active = Some(info);
        }

        if self.buf.extend_from_slice(data).is_err() {
            self.reset();
            return Err(ImageError::BufferFull);
        }

        self.expected_sequence = self.expected_sequence.wrapping_add(1);

        if info.is_last {
            Ok(Ingest::Complete)
        } else {
            Ok(Ingest::InProgress)
        }
    }
}

/// Panel index is carried in `ChunkInfo::slot`; clamp helper for sinks.
pub const fn panel_index(slot: u8) -> u8 {
    if slot as usize >= MAX_PANELS {
        0
    } else {
        slot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_two_chunks() {
        let mut a = ImageAssembler::<64>::new();
        let mut info = ChunkInfo {
            kind: ImageKind::Key,
            slot: 3,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            sequence: 0,
            is_last: false,
        };
        assert_eq!(a.ingest(info, b"abc"), Ok(Ingest::InProgress));
        info.sequence = 1;
        info.is_last = true;
        assert_eq!(a.ingest(info, b"def"), Ok(Ingest::Complete));
        assert_eq!(a.bytes(), b"abcdef");
    }

    #[test]
    fn bad_sequence_resets() {
        let mut a = ImageAssembler::<64>::new();
        let info = ChunkInfo {
            kind: ImageKind::Key,
            slot: 0,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            sequence: 0,
            is_last: false,
        };
        assert_eq!(a.ingest(info, b"a"), Ok(Ingest::InProgress));
        let bad = ChunkInfo {
            sequence: 5,
            ..info
        };
        assert_eq!(a.ingest(bad, b"b"), Err(ImageError::BadSequence));
        assert!(!a.is_active());
    }

    #[test]
    fn overflow_is_reported() {
        let mut a = ImageAssembler::<4>::new();
        let info = ChunkInfo {
            kind: ImageKind::Key,
            slot: 0,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            sequence: 0,
            is_last: true,
        };
        assert_eq!(a.ingest(info, b"abcde"), Err(ImageError::BufferFull));
    }
}
