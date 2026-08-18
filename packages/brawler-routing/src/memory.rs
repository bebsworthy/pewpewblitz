use std::collections::VecDeque;

use crate::{
    CONTROL_MAX_RECORD_BYTES, CodecError, PACKET_MAX_RECORD_BYTES,
    codec::{FramedDecoder, frame_record},
};

/// Deterministic byte-stream pair used by tests and the routed memory backend.
///
/// Writes use the exact u32 framing and reads use the same partial-record decoder required by
/// Unix streams. `chunk_bytes` controls deterministic short-write/read simulation.
#[derive(Clone)]
pub struct MemoryDuplex {
    maximum_record_bytes: usize,
    a_to_b: VecDeque<Vec<u8>>,
    b_to_a: VecDeque<Vec<u8>>,
    at_a: FramedDecoder,
    at_b: FramedDecoder,
}

impl MemoryDuplex {
    #[must_use]
    pub const fn new(maximum_record_bytes: usize) -> Self {
        Self {
            maximum_record_bytes,
            a_to_b: VecDeque::new(),
            b_to_a: VecDeque::new(),
            at_a: FramedDecoder::new(maximum_record_bytes),
            at_b: FramedDecoder::new(maximum_record_bytes),
        }
    }

    pub fn send_a_to_b(&mut self, record: &[u8], chunk_bytes: usize) -> Result<(), CodecError> {
        let framed = frame_record(record, self.maximum_record_bytes)?;
        enqueue_chunks(&mut self.a_to_b, &framed, chunk_bytes)
    }

    pub fn send_b_to_a(&mut self, record: &[u8], chunk_bytes: usize) -> Result<(), CodecError> {
        let framed = frame_record(record, self.maximum_record_bytes)?;
        enqueue_chunks(&mut self.b_to_a, &framed, chunk_bytes)
    }

    pub fn receive_at_a(&mut self) -> Result<Vec<Vec<u8>>, CodecError> {
        drain_chunks(&mut self.b_to_a, &mut self.at_a)
    }

    pub fn receive_at_b(&mut self) -> Result<Vec<Vec<u8>>, CodecError> {
        drain_chunks(&mut self.a_to_b, &mut self.at_b)
    }

    #[must_use]
    pub fn pending_chunks(&self) -> usize {
        self.a_to_b.len() + self.b_to_a.len()
    }
}

fn enqueue_chunks(
    queue: &mut VecDeque<Vec<u8>>,
    bytes: &[u8],
    chunk_bytes: usize,
) -> Result<(), CodecError> {
    if chunk_bytes == 0 {
        return Err(CodecError::InvalidValue);
    }
    queue.extend(bytes.chunks(chunk_bytes).map(<[u8]>::to_vec));
    Ok(())
}

fn drain_chunks(
    queue: &mut VecDeque<Vec<u8>>,
    decoder: &mut FramedDecoder,
) -> Result<Vec<Vec<u8>>, CodecError> {
    let mut records = Vec::new();
    while let Some(chunk) = queue.pop_front() {
        records.extend(decoder.push(&chunk)?);
    }
    Ok(records)
}

#[derive(Clone)]
pub struct MemoryBackend {
    pub packet: MemoryDuplex,
    pub control: MemoryDuplex,
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self {
            packet: MemoryDuplex::new(PACKET_MAX_RECORD_BYTES),
            control: MemoryDuplex::new(CONTROL_MAX_RECORD_BYTES),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{PacketDirection, PacketRecord, PeerId, RouteId, WorkerId};

    use super::*;

    #[test]
    fn memory_backend_preserves_exact_record_bytes_through_short_chunks() {
        let packet = PacketRecord::new(
            PacketDirection::SupervisorToWorker,
            WorkerId::new(1).unwrap(),
            RouteId::new(2).unwrap(),
            PeerId::new(3).unwrap(),
            vec![9; 200],
        )
        .unwrap();
        let encoded = packet.encode().unwrap();
        for chunk_bytes in 1..=encoded.len() + 4 {
            let mut backend = MemoryBackend::default();
            backend.packet.send_a_to_b(&encoded, chunk_bytes).unwrap();
            assert_eq!(
                backend.packet.receive_at_b().unwrap(),
                vec![encoded.clone()]
            );
            assert_eq!(backend.packet.pending_chunks(), 0);
        }
    }

    #[test]
    fn memory_backend_keeps_streams_and_directions_isolated() {
        let mut backend = MemoryBackend::default();
        backend.packet.send_a_to_b(b"packet", 2).unwrap();
        backend.control.send_b_to_a(b"control", 3).unwrap();
        assert!(backend.packet.receive_at_a().unwrap().is_empty());
        assert!(backend.control.receive_at_b().unwrap().is_empty());
        assert_eq!(
            backend.packet.receive_at_b().unwrap(),
            vec![b"packet".to_vec()]
        );
        assert_eq!(
            backend.control.receive_at_a().unwrap(),
            vec![b"control".to_vec()]
        );
    }
}
