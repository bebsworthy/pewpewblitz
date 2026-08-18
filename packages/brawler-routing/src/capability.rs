use std::fmt;

pub const CAPABILITY_BYTES: usize = 32;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Capability([u8; CAPABILITY_BYTES]);

impl Capability {
    pub fn generate() -> Result<Self, CapabilityEntropyError> {
        let mut bytes = [0_u8; CAPABILITY_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| CapabilityEntropyError)?;
        Self::from_bytes(bytes).ok_or(CapabilityEntropyError)
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; CAPABILITY_BYTES]) -> Option<Self> {
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != 0 {
                return Some(Self(bytes));
            }
            index += 1;
        }
        None
    }

    pub(crate) const fn expose_bytes(&self) -> &[u8; CAPABILITY_BYTES] {
        &self.0
    }

    /// Consume a capability to bridge it into another authenticated protocol representation.
    ///
    /// The returned bytes are secret material; callers must keep them out of logs and debug
    /// output.  `Capability` itself remains redacted by both `Debug` and `Display`.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; CAPABILITY_BYTES] {
        self.0
    }
}

impl fmt::Debug for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Capability([REDACTED])")
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityEntropyError;

impl fmt::Display for CapabilityEntropyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("operating-system capability entropy was unavailable")
    }
}

impl std::error::Error for CapabilityEntropyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_is_nonzero_and_always_redacted() {
        assert!(Capability::from_bytes([0; CAPABILITY_BYTES]).is_none());
        let capability = Capability::from_bytes([7; CAPABILITY_BYTES]).unwrap();
        assert_eq!(format!("{capability:?}"), "Capability([REDACTED])");
        assert_eq!(capability.to_string(), "[REDACTED]");
        assert!(!format!("{capability:?}").contains("07"));
    }

    #[test]
    fn generated_capability_is_nonzero() {
        assert!(Capability::generate().is_ok());
    }
}
