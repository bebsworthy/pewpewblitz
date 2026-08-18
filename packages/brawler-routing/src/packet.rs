use std::fmt;

use crate::{
    CodecError, INNER_MAX_DATAGRAM_BYTES, PACKET_HEADER_BYTES, PACKET_MAGIC,
    PACKET_MAX_RECORD_BYTES, PACKET_VERSION_V1, PeerId, RouteId, WorkerId,
    codec::{Decoder, Encoder, frame_record},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketDirection {
    SupervisorToWorker = 1,
    WorkerToSupervisor = 2,
}

impl TryFrom<u8> for PacketDirection {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::SupervisorToWorker),
            2 => Ok(Self::WorkerToSupervisor),
            other => Err(CodecError::UnsupportedType(other)),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PacketRecord {
    pub direction: PacketDirection,
    pub worker_id: WorkerId,
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub payload: Vec<u8>,
}

impl fmt::Debug for PacketRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PacketRecord")
            .field("direction", &self.direction)
            .field("worker_id", &self.worker_id)
            .field("route_id", &self.route_id)
            .field("peer_id", &self.peer_id)
            .field("payload_bytes", &self.payload.len())
            .finish()
    }
}

impl PacketRecord {
    pub fn new(
        direction: PacketDirection,
        worker_id: WorkerId,
        route_id: RouteId,
        peer_id: PeerId,
        payload: Vec<u8>,
    ) -> Result<Self, CodecError> {
        if payload.is_empty() {
            return Err(CodecError::InvalidValue);
        }
        if payload.len() > INNER_MAX_DATAGRAM_BYTES {
            return Err(CodecError::Oversize);
        }
        Ok(Self {
            direction,
            worker_id,
            route_id,
            peer_id,
            payload,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let mut encoder = Encoder::with_capacity(PACKET_HEADER_BYTES + self.payload.len());
        encoder.put_bytes(&PACKET_MAGIC);
        encoder.put_u8(PACKET_VERSION_V1);
        encoder.put_u8(self.direction as u8);
        encoder.put_u16(0);
        encoder.put_u128(self.worker_id.get());
        encoder.put_u128(self.route_id.get());
        encoder.put_u128(self.peer_id.get());
        encoder.put_u16(u16::try_from(self.payload.len()).map_err(|_| CodecError::Oversize)?);
        encoder.put_bytes(&self.payload);
        Ok(encoder.finish())
    }

    pub fn encode_framed(&self) -> Result<Vec<u8>, CodecError> {
        frame_record(&self.encode()?, PACKET_MAX_RECORD_BYTES)
    }

    pub fn decode(record: &[u8], expected_direction: PacketDirection) -> Result<Self, CodecError> {
        if record.len() > PACKET_MAX_RECORD_BYTES {
            return Err(CodecError::Oversize);
        }
        if record.len() < PACKET_HEADER_BYTES {
            return Err(CodecError::Truncated);
        }
        let mut decoder = Decoder::new(record);
        if decoder.take(4)? != PACKET_MAGIC {
            return Err(CodecError::InvalidMagic);
        }
        let version = decoder.u8()?;
        let direction_raw = decoder.u8()?;
        let flags = decoder.u16()?;
        let worker_raw = decoder.u128()?;
        let route_raw = decoder.u128()?;
        let peer_raw = decoder.u128()?;
        let payload_length = usize::from(decoder.u16()?);
        if payload_length == 0 {
            return Err(CodecError::InvalidValue);
        }
        if payload_length > INNER_MAX_DATAGRAM_BYTES {
            return Err(CodecError::Oversize);
        }
        if payload_length != decoder.remaining() {
            return Err(CodecError::LengthMismatch);
        }
        if version != PACKET_VERSION_V1 {
            return Err(CodecError::UnsupportedVersion(version));
        }
        let direction = PacketDirection::try_from(direction_raw)?;
        if flags != 0 {
            return Err(CodecError::ReservedNonZero);
        }
        let worker_id = WorkerId::new(worker_raw).ok_or(CodecError::ZeroId)?;
        let route_id = RouteId::new(route_raw).ok_or(CodecError::ZeroId)?;
        let peer_id = PeerId::new(peer_raw).ok_or(CodecError::ZeroId)?;
        let payload = decoder.take(payload_length)?.to_vec();
        decoder.finish()?;
        if direction != expected_direction {
            return Err(CodecError::InvalidValue);
        }
        Ok(Self {
            direction,
            worker_id,
            route_id,
            peer_id,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(length: usize) -> PacketRecord {
        PacketRecord::new(
            PacketDirection::SupervisorToWorker,
            WorkerId::new(1).unwrap(),
            RouteId::new(2).unwrap(),
            PeerId::new(3).unwrap(),
            vec![0xcc; length],
        )
        .unwrap()
    }

    #[test]
    fn exact_fixture_round_trip_and_framing() {
        let record = record(1);
        let encoded = record.encode().unwrap();
        assert_eq!(encoded.len(), 59);
        assert_eq!(&encoded[..8], b"BRPK\x01\x01\0\0");
        assert_eq!(&encoded[8..24], &1_u128.to_be_bytes());
        assert_eq!(&encoded[24..40], &2_u128.to_be_bytes());
        assert_eq!(&encoded[40..56], &3_u128.to_be_bytes());
        assert_eq!(&encoded[56..], &[0, 1, 0xcc]);
        assert_eq!(
            PacketRecord::decode(&encoded, PacketDirection::SupervisorToWorker).unwrap(),
            record
        );
        let framed = record.encode_framed().unwrap();
        assert_eq!(&framed[..4], &59_u32.to_be_bytes());
        assert_eq!(&framed[4..], encoded);
        let debug = format!("{record:?}");
        assert!(debug.contains("payload_bytes: 1"));
        assert!(!debug.contains("204"));
    }

    #[test]
    fn maximum_record_hits_exact_contract_boundary() {
        let record = record(INNER_MAX_DATAGRAM_BYTES);
        assert_eq!(record.encode().unwrap().len(), PACKET_MAX_RECORD_BYTES);
        assert_eq!(
            record.encode_framed().unwrap().len(),
            crate::PACKET_PREFIXED_MAX_BYTES
        );
        assert_eq!(
            PacketRecord::decode(&record.encode().unwrap(), record.direction).unwrap(),
            record
        );
        assert_eq!(
            PacketRecord::new(
                PacketDirection::SupervisorToWorker,
                WorkerId::new(1).unwrap(),
                RouteId::new(2).unwrap(),
                PeerId::new(3).unwrap(),
                vec![0; INNER_MAX_DATAGRAM_BYTES + 1]
            ),
            Err(CodecError::Oversize)
        );
    }

    #[test]
    fn every_header_truncation_is_rejected() {
        let encoded = record(1).encode().unwrap();
        for length in 0..PACKET_HEADER_BYTES {
            assert_eq!(
                PacketRecord::decode(&encoded[..length], PacketDirection::SupervisorToWorker),
                Err(CodecError::Truncated),
                "length {length}"
            );
        }
    }

    #[test]
    fn malformed_fields_and_endpoint_direction_are_rejected() {
        let valid = record(1).encode().unwrap();
        let decode =
            |bytes: &[u8]| PacketRecord::decode(bytes, PacketDirection::SupervisorToWorker);
        let mutate = |offset: usize, value: u8| {
            let mut bytes = valid.clone();
            bytes[offset] = value;
            bytes
        };
        assert_eq!(decode(&mutate(0, b'X')), Err(CodecError::InvalidMagic));
        assert_eq!(
            decode(&mutate(4, 2)),
            Err(CodecError::UnsupportedVersion(2))
        );
        assert_eq!(decode(&mutate(5, 9)), Err(CodecError::UnsupportedType(9)));
        assert_eq!(decode(&mutate(7, 1)), Err(CodecError::ReservedNonZero));
        assert_eq!(decode(&mutate(5, 2)), Err(CodecError::InvalidValue));
        for id_offset in [8, 24, 40] {
            let mut bytes = valid.clone();
            bytes[id_offset..id_offset + 16].fill(0);
            assert_eq!(decode(&bytes), Err(CodecError::ZeroId));
        }
        let mut mismatch = valid.clone();
        mismatch[57] = 2;
        assert_eq!(decode(&mismatch), Err(CodecError::LengthMismatch));
        let mut empty = valid.clone();
        empty.truncate(PACKET_HEADER_BYTES);
        empty[57] = 0;
        assert_eq!(decode(&empty), Err(CodecError::InvalidValue));
        assert_eq!(
            decode(&vec![0; PACKET_MAX_RECORD_BYTES + 1]),
            Err(CodecError::Oversize)
        );
    }
}
