//! Shared bounded primitives for the explicit routing codecs.

use crate::CodecError;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    pub fn put_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }
    pub fn put_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    pub fn put_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    pub fn put_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    pub fn put_u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    pub fn put_bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    pub fn put_string(&mut self, value: &str, maximum: usize) -> Result<(), CodecError> {
        if value.len() > maximum || value.len() > usize::from(u16::MAX) {
            return Err(CodecError::Oversize);
        }
        self.put_u16(u16::try_from(value.len()).map_err(|_| CodecError::Oversize)?);
        self.put_bytes(value.as_bytes());
        Ok(())
    }

    pub fn put_blob(&mut self, value: &[u8], maximum: usize) -> Result<(), CodecError> {
        if value.len() > maximum || value.len() > u32::MAX as usize {
            return Err(CodecError::Oversize);
        }
        self.put_u32(u32::try_from(value.len()).map_err(|_| CodecError::Oversize)?);
        self.put_bytes(value);
        Ok(())
    }

    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Decoder<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Decoder<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    pub fn take(&mut self, count: usize) -> Result<&'a [u8], CodecError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(CodecError::Oversize)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(CodecError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_be_bytes(
            self.take(2)?.try_into().expect("exact width"),
        ))
    }
    pub fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().expect("exact width"),
        ))
    }
    pub fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().expect("exact width"),
        ))
    }
    pub fn u128(&mut self) -> Result<u128, CodecError> {
        Ok(u128::from_be_bytes(
            self.take(16)?.try_into().expect("exact width"),
        ))
    }

    pub fn string(&mut self, maximum: usize) -> Result<&'a str, CodecError> {
        let length = usize::from(self.u16()?);
        if length > maximum {
            return Err(CodecError::Oversize);
        }
        std::str::from_utf8(self.take(length)?).map_err(|_| CodecError::InvalidUtf8)
    }

    pub fn blob(&mut self, maximum: usize) -> Result<&'a [u8], CodecError> {
        let length = usize::try_from(self.u32()?).map_err(|_| CodecError::Oversize)?;
        if length > maximum {
            return Err(CodecError::Oversize);
        }
        self.take(length)
    }

    pub fn boolean(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CodecError::InvalidValue),
        }
    }

    pub fn optional<T>(
        &mut self,
        decode: impl FnOnce(&mut Self) -> Result<T, CodecError>,
    ) -> Result<Option<T>, CodecError> {
        match self.u8()? {
            0 => Ok(None),
            1 => decode(self).map(Some),
            _ => Err(CodecError::InvalidValue),
        }
    }

    pub fn finish(self) -> Result<(), CodecError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(CodecError::TrailingData)
        }
    }
}

/// Stateful u32-big-endian record decoder for nonblocking byte streams.
#[derive(Clone, Debug)]
pub struct FramedDecoder {
    maximum_record_bytes: usize,
    bytes: Vec<u8>,
}

impl FramedDecoder {
    #[must_use]
    pub const fn new(maximum_record_bytes: usize) -> Self {
        Self {
            maximum_record_bytes,
            bytes: Vec::new(),
        }
    }

    pub fn push(&mut self, incoming: &[u8]) -> Result<Vec<Vec<u8>>, CodecError> {
        let mut records = Vec::new();

        // Inspect and retain only the four-byte prefix before accepting any body bytes.  A
        // readiness read can hand us an arbitrarily large chunk; extending `bytes` with that
        // chunk before validating its prefix would let an invalid advertised length consume
        // unbounded memory.  The loop also ensures that a partial frame never retains more than
        // its validated four-byte prefix plus the bounded body.
        let mut offset = 0;
        while offset < incoming.len() || !self.bytes.is_empty() {
            if self.bytes.len() < 4 {
                let prefix_bytes = (4 - self.bytes.len()).min(incoming.len() - offset);
                self.bytes
                    .extend_from_slice(&incoming[offset..offset + prefix_bytes]);
                offset += prefix_bytes;
                if self.bytes.len() < 4 {
                    break;
                }
            }

            let advertised = u32::from_be_bytes(self.bytes[..4].try_into().expect("exact width"));
            let record_len = usize::try_from(advertised).map_err(|_| CodecError::Oversize)?;
            if record_len == 0 {
                return Err(CodecError::InvalidValue);
            }
            if record_len > self.maximum_record_bytes {
                return Err(CodecError::Oversize);
            }
            let framed_len = 4_usize
                .checked_add(record_len)
                .ok_or(CodecError::Oversize)?;

            let body_bytes = framed_len.saturating_sub(self.bytes.len());
            let incoming_bytes = (incoming.len() - offset).min(body_bytes);
            self.bytes
                .extend_from_slice(&incoming[offset..offset + incoming_bytes]);
            offset += incoming_bytes;
            if self.bytes.len() < framed_len {
                break;
            }
            let frame = self.bytes[4..framed_len].to_vec();
            self.bytes.clear();
            records.push(frame);
        }
        Ok(records)
    }

    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.bytes.len()
    }
}

pub fn frame_record(record: &[u8], maximum_record_bytes: usize) -> Result<Vec<u8>, CodecError> {
    if record.is_empty() {
        return Err(CodecError::InvalidValue);
    }
    if record.len() > maximum_record_bytes || record.len() > u32::MAX as usize {
        return Err(CodecError::Oversize);
    }
    let mut framed = Vec::with_capacity(4 + record.len());
    framed.extend_from_slice(
        &u32::try_from(record.len())
            .map_err(|_| CodecError::Oversize)?
            .to_be_bytes(),
    );
    framed.extend_from_slice(record);
    Ok(framed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_are_big_endian_and_bounded() {
        let mut encoder = Encoder::new();
        encoder.put_u16(0x1234);
        encoder.put_u32(0x5678_9abc);
        encoder.put_string("é", 2).unwrap();
        encoder.put_blob(&[4, 5], 2).unwrap();
        let bytes = encoder.finish();
        assert_eq!(&bytes[..6], &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]);
        let mut decoder = Decoder::new(&bytes);
        assert_eq!(decoder.u16().unwrap(), 0x1234);
        assert_eq!(decoder.u32().unwrap(), 0x5678_9abc);
        assert_eq!(decoder.string(2).unwrap(), "é");
        assert_eq!(decoder.blob(2).unwrap(), &[4, 5]);
        decoder.finish().unwrap();
    }

    #[test]
    fn primitives_reject_malformed_tags_utf8_and_trailing_data() {
        assert_eq!(Decoder::new(&[2]).boolean(), Err(CodecError::InvalidValue));
        assert_eq!(
            Decoder::new(&[2]).optional(|_| Ok(())).unwrap_err(),
            CodecError::InvalidValue
        );
        assert_eq!(
            Decoder::new(&[0, 1, 0xff]).string(1),
            Err(CodecError::InvalidUtf8)
        );
        assert_eq!(Decoder::new(&[1]).finish(), Err(CodecError::TrailingData));
    }

    #[test]
    fn framed_decoder_handles_every_split_and_multiple_frames() {
        let first = frame_record(b"alpha", 8).unwrap();
        let second = frame_record(b"b", 8).unwrap();
        let mut all = first.clone();
        all.extend_from_slice(&second);
        for split in 0..=all.len() {
            let mut decoder = FramedDecoder::new(8);
            let mut records = decoder.push(&all[..split]).unwrap();
            records.extend(decoder.push(&all[split..]).unwrap());
            assert_eq!(records, vec![b"alpha".to_vec(), b"b".to_vec()]);
            assert_eq!(decoder.buffered_bytes(), 0);
        }
    }

    #[test]
    fn framed_decoder_rejects_zero_and_oversize_before_body() {
        assert_eq!(
            FramedDecoder::new(8).push(&0_u32.to_be_bytes()),
            Err(CodecError::InvalidValue)
        );
        assert_eq!(
            FramedDecoder::new(8).push(&9_u32.to_be_bytes()),
            Err(CodecError::Oversize)
        );
        assert_eq!(frame_record(&[], 8), Err(CodecError::InvalidValue));
        assert_eq!(frame_record(&[0; 9], 8), Err(CodecError::Oversize));
    }

    #[test]
    fn framed_decoder_does_not_retain_huge_chunk_for_oversize_prefix() {
        let mut incoming = vec![0_u8; 4 + 1024 * 1024];
        incoming[..4].copy_from_slice(&9_u32.to_be_bytes());
        let mut decoder = FramedDecoder::new(8);
        assert_eq!(decoder.push(&incoming), Err(CodecError::Oversize));
        assert_eq!(decoder.buffered_bytes(), 4);
    }

    #[test]
    fn framed_decoder_processes_large_chunk_in_bounded_frames() {
        let mut incoming = Vec::new();
        for value in 0_u8..=127 {
            incoming.extend_from_slice(&frame_record(&[value], 8).unwrap());
        }
        let mut decoder = FramedDecoder::new(8);
        let records = decoder.push(&incoming).unwrap();
        assert_eq!(records.len(), 128);
        assert_eq!(records.first(), Some(&vec![0]));
        assert_eq!(records.last(), Some(&vec![127]));
        assert_eq!(decoder.buffered_bytes(), 0);
    }
}
