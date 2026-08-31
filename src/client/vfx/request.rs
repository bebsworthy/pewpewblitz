//! Renderer-neutral client VFX request contracts.

use bevy::prelude::{Message, Vec2};

pub(crate) const MAX_VFX_REQUEST_KEY_LENGTH: usize = 64;
const MAX_VFX_DIAGNOSTIC_LABEL_LENGTH: usize = 96;

pub(crate) const COMBAT_VFX_PRODUCER_RANK: u16 = 100;
pub(crate) const WORLD_OBJECT_VFX_PRODUCER_RANK: u16 = 200;
pub(crate) const PICKUP_VFX_PRODUCER_RANK: u16 = 300;
pub(crate) const HEIST_VFX_PRODUCER_RANK: u16 = 400;

pub(crate) const COMBAT_MUZZLE_VFX: VfxRequestKey = VfxRequestKey::new("combat.muzzle");
pub(crate) const COMBAT_IMPACT_VFX: VfxRequestKey = VfxRequestKey::new("combat.impact");
pub(crate) const COMBAT_DAMAGE_VFX: VfxRequestKey = VfxRequestKey::new("combat.damage");
pub(crate) const COMBAT_RESET_VFX: VfxRequestKey = VfxRequestKey::new("combat.reset");
pub(crate) const REVEAL_SCAN_VFX: VfxRequestKey = VfxRequestKey::new("ability.reveal-scan");
pub(crate) const ELEMENTAL_FIELD_VFX: VfxRequestKey = VfxRequestKey::new("ability.elemental-field");
pub(crate) const DEMOLITION_STRIKE_VFX: VfxRequestKey =
    VfxRequestKey::new("ability.demolition-strike");
pub(crate) const WORLD_OBJECT_DAMAGED_VFX: VfxRequestKey =
    VfxRequestKey::new("world-object.damaged");
pub(crate) const WORLD_OBJECT_EXPLOSION_VFX: VfxRequestKey =
    VfxRequestKey::new("world-object.explosion");
pub(crate) const PICKUP_SPAWNED_VFX: VfxRequestKey = VfxRequestKey::new("pickup.spawned");
pub(crate) const PICKUP_COLLECTED_VFX: VfxRequestKey = VfxRequestKey::new("pickup.collected");
pub(crate) const PICKUP_EXPIRED_VFX: VfxRequestKey = VfxRequestKey::new("pickup.expired");
pub(crate) const HEIST_DAMAGED_VFX: VfxRequestKey = VfxRequestKey::new("heist.damaged");
pub(crate) const HEIST_CRITICAL_VFX: VfxRequestKey = VfxRequestKey::new("heist.critical");
pub(crate) const HEIST_DESTROYED_VFX: VfxRequestKey = VfxRequestKey::new("heist.destroyed");

/// A process-local semantic presentation identity registered by one feature plugin.
///
/// Keys are deliberately static and bounded. They identify client presentation policy only and
/// never enter the wire protocol or shared gameplay-content fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct VfxRequestKey(&'static str);

impl VfxRequestKey {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }

    pub(super) fn is_valid(self) -> bool {
        valid_vfx_request_key(self.0)
    }
}

/// Stable ordering material carried independently of Bevy's parallel message merge order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct VfxRequestOrder {
    pub(crate) producer_rank: u16,
    pub(crate) event_id: u64,
}

impl VfxRequestOrder {
    pub(crate) const fn new(producer_rank: u16, event_id: u64) -> Self {
        Self {
            producer_rank,
            event_id,
        }
    }
}

/// Authoritative timing context for a presentation profile whose lifetime mirrors gameplay.
#[allow(
    clippy::struct_field_names,
    reason = "the explicit tick suffix distinguishes authoritative simulation time from wall time"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VfxDeadline {
    pub(crate) activated_at_tick: u64,
    pub(crate) expires_at_tick: u64,
    pub(crate) observed_at_tick: Option<u64>,
}

impl VfxDeadline {
    pub(crate) const fn new(
        activated_at_tick: u64,
        expires_at_tick: u64,
        observed_at_tick: Option<u64>,
    ) -> Self {
        Self {
            activated_at_tick,
            expires_at_tick,
            observed_at_tick,
        }
    }

    pub(super) const fn is_valid(self) -> bool {
        self.activated_at_tick <= self.expires_at_tick
    }
}

/// One feature-owned semantic request for a transient client-only visual effect.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub(crate) struct VfxRequest {
    pub(crate) key: VfxRequestKey,
    pub(crate) order: VfxRequestOrder,
    pub(crate) position: Vec2,
    pub(crate) authoritative_radius: Option<f32>,
    pub(crate) deadline: Option<VfxDeadline>,
    pub(crate) label: &'static str,
}

impl VfxRequest {
    pub(crate) fn try_new(
        key: VfxRequestKey,
        order: VfxRequestOrder,
        position: Vec2,
        authoritative_radius: Option<f32>,
        deadline: Option<VfxDeadline>,
        label: &'static str,
    ) -> Result<Self, String> {
        let request = Self {
            key,
            order,
            position,
            authoritative_radius,
            deadline,
            label,
        };
        request.validate()?;
        Ok(request)
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if !self.key.is_valid() {
            return Err(format!("invalid VFX request key: {}", self.key.as_str()));
        }
        if !self.position.is_finite() {
            return Err("VFX request position must be finite".into());
        }
        if self
            .authoritative_radius
            .is_some_and(|radius| !radius.is_finite() || radius <= 0.0)
        {
            return Err("VFX request authoritative radius must be finite and positive".into());
        }
        if self.deadline.is_some_and(|deadline| !deadline.is_valid()) {
            return Err("VFX request deadline expires before activation".into());
        }
        if self.label.is_empty() || self.label.len() > MAX_VFX_DIAGNOSTIC_LABEL_LENGTH {
            return Err("VFX request diagnostic label is empty or oversized".into());
        }
        Ok(())
    }
}

pub(super) fn valid_vfx_request_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_VFX_REQUEST_KEY_LENGTH
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_validation_accepts_typed_geometry_and_deadline() {
        let request = VfxRequest::try_new(
            VfxRequestKey::new("ability.reveal-scan"),
            VfxRequestOrder::new(10, 42),
            Vec2::new(20.0, 30.0),
            Some(64.0),
            Some(VfxDeadline::new(100, 400, Some(120))),
            "Reveal Scan area",
        )
        .unwrap();

        assert_eq!(request.key.as_str(), "ability.reveal-scan");
        assert_eq!(request.order.event_id, 42);
    }

    #[test]
    fn request_validation_rejects_unbounded_or_invalid_payloads() {
        for key in ["", "Combat.Muzzle", "combat/muzzle"] {
            assert!(
                VfxRequest::try_new(
                    VfxRequestKey::new(key),
                    VfxRequestOrder::new(1, 1),
                    Vec2::ZERO,
                    None,
                    None,
                    "test",
                )
                .is_err()
            );
        }
        assert!(
            VfxRequest::try_new(
                VfxRequestKey::new("combat.muzzle"),
                VfxRequestOrder::new(1, 1),
                Vec2::new(f32::NAN, 0.0),
                None,
                None,
                "test",
            )
            .is_err()
        );
        assert!(
            VfxRequest::try_new(
                VfxRequestKey::new("combat.muzzle"),
                VfxRequestOrder::new(1, 1),
                Vec2::ZERO,
                Some(0.0),
                None,
                "test",
            )
            .is_err()
        );
        assert!(
            VfxRequest::try_new(
                VfxRequestKey::new("combat.muzzle"),
                VfxRequestOrder::new(1, 1),
                Vec2::ZERO,
                None,
                Some(VfxDeadline::new(2, 1, None)),
                "test",
            )
            .is_err()
        );
    }
}
