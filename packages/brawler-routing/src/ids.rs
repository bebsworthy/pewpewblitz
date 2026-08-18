use std::{
    fmt,
    num::{NonZeroU64, NonZeroU128},
};

macro_rules! nonzero_id {
    ($name:ident, $inner:ty, $primitive:ty) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name($inner);

        impl $name {
            pub const fn new(value: $primitive) -> Option<Self> {
                match <$inner>::new(value) {
                    Some(value) => Some(Self(value)),
                    None => None,
                }
            }

            pub const fn get(self) -> $primitive {
                self.0.get()
            }
        }

        impl From<$name> for $primitive {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl TryFrom<$primitive> for $name {
            type Error = ZeroIdError;

            fn try_from(value: $primitive) -> Result<Self, Self::Error> {
                Self::new(value).ok_or(ZeroIdError)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.get())
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.get().fmt(formatter)
            }
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZeroIdError;

impl fmt::Display for ZeroIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("identifier must be nonzero")
    }
}

impl std::error::Error for ZeroIdError {}

nonzero_id!(PeerId, NonZeroU128, u128);
nonzero_id!(RouteId, NonZeroU128, u128);
nonzero_id!(WorkerId, NonZeroU128, u128);
nonzero_id!(ProcessId, NonZeroU128, u128);
nonzero_id!(LobbySessionId, NonZeroU128, u128);
nonzero_id!(AllocationId, NonZeroU128, u128);
nonzero_id!(MatchId, NonZeroU128, u128);
nonzero_id!(LogicalServerId, NonZeroU128, u128);

nonzero_id!(PlayerId, NonZeroU64, u64);
nonzero_id!(NetcodeClientId, NonZeroU64, u64);
nonzero_id!(RequestId, NonZeroU64, u64);
nonzero_id!(StopId, NonZeroU64, u64);
nonzero_id!(Generation, NonZeroU64, u64);
nonzero_id!(Sequence, NonZeroU64, u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_ids_reject_zero_and_preserve_values() {
        assert_eq!(RouteId::new(0), None);
        let id = RouteId::new(u128::MAX).unwrap();
        assert_eq!(id.get(), u128::MAX);
        assert_eq!(id.to_string(), u128::MAX.to_string());
        assert_eq!(RequestId::new(0), None);
        assert_eq!(RequestId::new(u64::MAX).unwrap().get(), u64::MAX);
        assert_eq!(NetcodeClientId::new(0), None);
        assert_eq!(NetcodeClientId::new(u64::MAX).unwrap().get(), u64::MAX);
    }
}
