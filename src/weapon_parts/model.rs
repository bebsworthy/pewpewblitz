use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

pub const WEAPON_PART_SLOT_COUNT: usize = 4;
pub const MAX_WEAPON_PARTS_PER_PROFILE: usize = 128;
pub const MAX_PART_EFFECTS_PER_INSTANCE: usize = 4;
pub const MAX_PART_NAME_BYTES: usize = 64;
pub const MAX_PART_TYPE_BYTES: usize = 32;
pub const MAX_PART_EFFECT_PAYLOAD_BYTES: usize = 128;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WeaponPartDefinitionId(pub u16);

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WeaponPartInstanceId(u128);

impl<'de> Deserialize<'de> for WeaponPartInstanceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u128::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl WeaponPartInstanceId {
    pub fn new(value: u128) -> Result<Self, WeaponPartModelError> {
        (value != 0)
            .then_some(Self(value))
            .ok_or(WeaponPartModelError::ZeroId)
    }

    pub fn random() -> Result<Self, WeaponPartModelError> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| WeaponPartModelError::EntropyUnavailable)?;
        Self::new(u128::from_be_bytes(bytes))
    }

    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }

    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, WeaponPartModelError> {
        Self::new(u128::from_be_bytes(bytes))
    }
}

impl fmt::Display for WeaponPartInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

impl fmt::Debug for WeaponPartInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "WeaponPartInstanceId({self})")
    }
}

impl FromStr for WeaponPartInstanceId {
    type Err = WeaponPartModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(WeaponPartModelError::MalformedId);
        }
        Self::new(u128::from_str_radix(value, 16).map_err(|_| WeaponPartModelError::MalformedId)?)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum WeaponPartEffect {
    Capacity {
        flat: i8,
        percent_basis_points: i16,
    },
    Damage {
        flat: i16,
        percent_basis_points: i16,
    },
    FireInterval {
        flat_ticks: i16,
        percent_basis_points: i16,
    },
    RefillInterval {
        flat_ticks: i16,
        percent_basis_points: i16,
    },
    Reach {
        flat_milliunits: i32,
        percent_basis_points: i16,
    },
    Slow {
        penalty_basis_points: u16,
        duration_ticks: u16,
    },
    Cold {
        amount: u16,
    },
    DamageOverTime {
        kind: crate::combat::DamageOverTimeKind,
        damage_per_tick: u16,
        tick_interval: u16,
        duration_ticks: u16,
    },
    Heal {
        amount: u16,
    },
}

impl WeaponPartEffect {
    pub fn validate(self) -> Result<(), WeaponPartModelError> {
        let valid_percent = |value: i16| (-5_000..=5_000).contains(&value);
        let valid = match self {
            Self::Capacity {
                flat,
                percent_basis_points,
            } => (flat != 0 || percent_basis_points != 0) && valid_percent(percent_basis_points),
            Self::Damage {
                flat,
                percent_basis_points,
            }
            | Self::FireInterval {
                flat_ticks: flat,
                percent_basis_points,
            }
            | Self::RefillInterval {
                flat_ticks: flat,
                percent_basis_points,
            } => (flat != 0 || percent_basis_points != 0) && valid_percent(percent_basis_points),
            Self::Reach {
                flat_milliunits,
                percent_basis_points,
            } => {
                (flat_milliunits != 0 || percent_basis_points != 0)
                    && flat_milliunits.abs() <= 1_000_000
                    && valid_percent(percent_basis_points)
            }
            Self::Slow {
                penalty_basis_points,
                duration_ticks,
            } => {
                (1..=6_000).contains(&penalty_basis_points) && (1..=3_600).contains(&duration_ticks)
            }
            Self::Cold { amount } | Self::Heal { amount } => (1..=1_000).contains(&amount),
            Self::DamageOverTime {
                damage_per_tick,
                tick_interval,
                duration_ticks,
                ..
            } => {
                (1..=1_000).contains(&damage_per_tick)
                    && (1..=3_600).contains(&tick_interval)
                    && tick_interval <= duration_ticks
                    && (1..=3_600).contains(&duration_ticks)
            }
        };
        valid
            .then_some(())
            .ok_or(WeaponPartModelError::InvalidEffect)
    }

    #[must_use]
    pub const fn kind_ordinal(self) -> u8 {
        match self {
            Self::Capacity { .. } => 0,
            Self::Damage { .. } => 1,
            Self::FireInterval { .. } => 2,
            Self::RefillInterval { .. } => 3,
            Self::Reach { .. } => 4,
            Self::Slow { .. } => 5,
            Self::Cold { .. } => 6,
            Self::DamageOverTime {
                kind: crate::combat::DamageOverTimeKind::Poison,
                ..
            } => 7,
            Self::DamageOverTime {
                kind: crate::combat::DamageOverTimeKind::Fire,
                ..
            } => 8,
            Self::Heal { .. } => 9,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WeaponPartInstance {
    pub id: WeaponPartInstanceId,
    pub inventory_ordinal: u64,
    pub definition_id: WeaponPartDefinitionId,
    pub display_name: String,
    pub effects: Vec<WeaponPartEffect>,
}

impl WeaponPartInstance {
    pub fn validate(&self) -> Result<(), WeaponPartModelError> {
        if self.inventory_ordinal == 0
            || self.definition_id.0 == 0
            || !valid_text(&self.display_name, MAX_PART_NAME_BYTES)
            || self.effects.is_empty()
            || self.effects.len() > MAX_PART_EFFECTS_PER_INSTANCE
            || self
                .effects
                .windows(2)
                .any(|pair| pair[0].kind_ordinal() >= pair[1].kind_ordinal())
            || self.effects.iter().any(|effect| effect.validate().is_err())
            || postcard::to_allocvec(&self.effects)
                .map_or(true, |bytes| bytes.len() > MAX_PART_EFFECT_PAYLOAD_BYTES)
        {
            return Err(WeaponPartModelError::InvalidInstance);
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanonicalScalarModifier {
    pub flat: i32,
    pub percent_basis_points: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanonicalSlowModifier {
    pub penalty_basis_points: u16,
    pub duration_ticks: u16,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanonicalWeaponModifiers {
    pub capacity: CanonicalScalarModifier,
    pub damage: CanonicalScalarModifier,
    pub fire_interval: CanonicalScalarModifier,
    pub refill_interval: CanonicalScalarModifier,
    pub reach_milliunits: CanonicalScalarModifier,
    pub slow: Option<CanonicalSlowModifier>,
    pub cold: Option<u16>,
    pub poison: Option<CanonicalDamageOverTimeModifier>,
    pub fire: Option<CanonicalDamageOverTimeModifier>,
    pub heal: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanonicalDamageOverTimeModifier {
    pub damage_per_tick: u16,
    pub tick_interval: u16,
    pub duration_ticks: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeaponPartModelError {
    ZeroId,
    MalformedId,
    EntropyUnavailable,
    InvalidEffect,
    InvalidInstance,
    TooManyParts,
    DuplicateInstance,
    ArithmeticOverflow,
    IncompatibleWeapon,
}

impl fmt::Display for WeaponPartModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

pub(crate) fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}
