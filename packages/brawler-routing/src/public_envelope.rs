use std::fmt;

use crate::{
    Capability, CodecError, INNER_MAX_DATAGRAM_BYTES, PUBLIC_HEADER_BYTES, PUBLIC_MAGIC,
    PUBLIC_MAX_DATAGRAM_BYTES, ROUTE_VERSION_V1,
    codec::{Decoder, Encoder},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteSelector {
    DefaultLobby,
    Capability(Capability),
}

#[derive(Clone, PartialEq, Eq)]
pub struct PublicEnvelope {
    selector: RouteSelector,
    payload: Vec<u8>,
}

impl fmt::Debug for PublicEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicEnvelope")
            .field("selector", &self.selector)
            .field("payload_bytes", &self.payload.len())
            .finish()
    }
}

impl PublicEnvelope {
    pub fn new(selector: RouteSelector, payload: Vec<u8>) -> Result<Self, CodecError> {
        if payload.is_empty() {
            return Err(CodecError::InvalidValue);
        }
        if payload.len() > INNER_MAX_DATAGRAM_BYTES {
            return Err(CodecError::Oversize);
        }
        Ok(Self { selector, payload })
    }

    #[must_use]
    pub const fn selector(&self) -> &RouteSelector {
        &self.selector
    }

    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let mut encoder = Encoder::with_capacity(PUBLIC_HEADER_BYTES + self.payload.len());
        encoder.put_bytes(&PUBLIC_MAGIC);
        encoder.put_u8(ROUTE_VERSION_V1);
        match &self.selector {
            RouteSelector::DefaultLobby => {
                encoder.put_u8(1);
                encoder.put_u16(0);
                encoder.put_bytes(&[0; 32]);
            }
            RouteSelector::Capability(capability) => {
                encoder.put_u8(2);
                encoder.put_u16(0);
                encoder.put_bytes(capability.expose_bytes());
            }
        }
        encoder.put_u16(u16::try_from(self.payload.len()).map_err(|_| CodecError::Oversize)?);
        encoder.put_bytes(&self.payload);
        let datagram = encoder.finish();
        debug_assert!(datagram.len() <= PUBLIC_MAX_DATAGRAM_BYTES);
        Ok(datagram)
    }

    pub fn decode(datagram: &[u8]) -> Result<Self, CodecError> {
        if datagram.len() > PUBLIC_MAX_DATAGRAM_BYTES {
            return Err(CodecError::Oversize);
        }
        if datagram.len() < PUBLIC_HEADER_BYTES {
            return Err(CodecError::Truncated);
        }
        let mut decoder = Decoder::new(datagram);
        if decoder.take(4)? != PUBLIC_MAGIC {
            return Err(CodecError::InvalidMagic);
        }
        let version = decoder.u8()?;
        let kind = decoder.u8()?;
        let flags = decoder.u16()?;
        let selector_bytes: [u8; 32] = decoder.take(32)?.try_into().expect("exact width");
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
        if version != ROUTE_VERSION_V1 {
            return Err(CodecError::UnsupportedVersion(version));
        }
        if !matches!(kind, 1 | 2) {
            return Err(CodecError::UnsupportedType(kind));
        }
        if flags != 0 {
            return Err(CodecError::ReservedNonZero);
        }
        let selector = match kind {
            1 => {
                if selector_bytes != [0; 32] {
                    return Err(CodecError::InvalidValue);
                }
                RouteSelector::DefaultLobby
            }
            2 => RouteSelector::Capability(
                Capability::from_bytes(selector_bytes).ok_or(CodecError::InvalidValue)?,
            ),
            _ => unreachable!("kind was validated"),
        };
        let payload = decoder.take(payload_length)?.to_vec();
        decoder.finish()?;
        Ok(Self { selector, payload })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(selector: RouteSelector, length: usize) -> PublicEnvelope {
        PublicEnvelope::new(selector, vec![0xa5; length]).unwrap()
    }

    #[test]
    fn exact_lobby_fixture_and_round_trip() {
        let encoded = fixture(RouteSelector::DefaultLobby, 1).encode().unwrap();
        assert_eq!(encoded.len(), 43);
        assert_eq!(&encoded[..8], b"BRTE\x01\x01\0\0");
        assert_eq!(&encoded[8..40], &[0; 32]);
        assert_eq!(&encoded[40..], &[0, 1, 0xa5]);
        assert_eq!(
            PublicEnvelope::decode(&encoded).unwrap(),
            fixture(RouteSelector::DefaultLobby, 1)
        );
        let debug = format!("{:?}", fixture(RouteSelector::DefaultLobby, 1));
        assert!(debug.contains("payload_bytes: 1"));
        assert!(!debug.contains("165"));
    }

    #[test]
    fn capability_round_trip_and_maximum_boundary() {
        let capability = Capability::from_bytes([0x5a; 32]).unwrap();
        let envelope = fixture(
            RouteSelector::Capability(capability),
            INNER_MAX_DATAGRAM_BYTES,
        );
        let encoded = envelope.encode().unwrap();
        assert_eq!(encoded.len(), PUBLIC_MAX_DATAGRAM_BYTES);
        assert_eq!(PublicEnvelope::decode(&encoded).unwrap(), envelope);
        assert_eq!(
            PublicEnvelope::new(
                RouteSelector::DefaultLobby,
                vec![0; INNER_MAX_DATAGRAM_BYTES + 1]
            ),
            Err(CodecError::Oversize)
        );
    }

    #[test]
    fn every_header_truncation_is_rejected() {
        let encoded = fixture(RouteSelector::DefaultLobby, 1).encode().unwrap();
        for length in 0..PUBLIC_HEADER_BYTES {
            assert_eq!(
                PublicEnvelope::decode(&encoded[..length]),
                Err(CodecError::Truncated),
                "length {length}"
            );
        }
    }

    #[test]
    fn malformed_and_unsupported_fields_are_distinct() {
        let valid = fixture(RouteSelector::DefaultLobby, 1).encode().unwrap();
        let mutate = |offset: usize, value: u8| {
            let mut bytes = valid.clone();
            bytes[offset] = value;
            bytes
        };
        assert_eq!(
            PublicEnvelope::decode(&mutate(0, b'X')),
            Err(CodecError::InvalidMagic)
        );
        assert_eq!(
            PublicEnvelope::decode(&mutate(4, 2)),
            Err(CodecError::UnsupportedVersion(2))
        );
        assert_eq!(
            PublicEnvelope::decode(&mutate(5, 9)),
            Err(CodecError::UnsupportedType(9))
        );
        assert_eq!(
            PublicEnvelope::decode(&mutate(7, 1)),
            Err(CodecError::ReservedNonZero)
        );
        assert_eq!(
            PublicEnvelope::decode(&mutate(8, 1)),
            Err(CodecError::InvalidValue)
        );
        let mut zero_capability = valid.clone();
        zero_capability[5] = 2;
        assert_eq!(
            PublicEnvelope::decode(&zero_capability),
            Err(CodecError::InvalidValue)
        );
        let mut wrong_length = valid.clone();
        wrong_length[41] = 2;
        assert_eq!(
            PublicEnvelope::decode(&wrong_length),
            Err(CodecError::LengthMismatch)
        );
        let mut empty = valid.clone();
        empty.truncate(PUBLIC_HEADER_BYTES);
        empty[41] = 0;
        assert_eq!(
            PublicEnvelope::decode(&empty),
            Err(CodecError::InvalidValue)
        );
        assert_eq!(
            PublicEnvelope::decode(&vec![0; PUBLIC_MAX_DATAGRAM_BYTES + 1]),
            Err(CodecError::Oversize)
        );
    }
}
