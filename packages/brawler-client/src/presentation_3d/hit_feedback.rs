//! Presentation-only hit feedback: a short white flash on whatever was damaged and pooled floating
//! damage/healing numbers, including healing recovered outside healing cues (`recovery`).
//!
//! Every flash and number comes from an authoritative replicated cue (`DeduplicatedCombatCue`,
//! `ReceivedWorldObjectCues`, `ReceivedVaultBreakObjectiveCue`); nothing here computes damage or writes
//! gameplay state. The server already withholds cues about fighters concealed from this observer.
//! As a fail-closed second gate, a cue whose target is not replicated to this observer produces
//! neither a flash nor a number. Defeat feedback stays on the shared VFX/audio request path
//! (authored `defeat` recipes and the `defeat` audio cue), and camera shake stays with the rig.

use super::{
    ArenaCamera, CameraUpdateSystems, ClientShellSettings, CombatVisualOwner, Controlled,
    DynamicMapVisual, FIGHTER_VISUAL_HEIGHT_WORLD, Fighter, NetworkEntityId, ShadedMaterial,
    ToonMaterial, UiSystems, V3FighterVisual, VaultBreakSafeVisual, WorldPresentationSet, combat,
    ground_position, ui, update_damageable_map_visuals, update_vault_break_safe_status_visuals,
    vfx, zombie,
};
use bevy::prelude::*;
use brawler::combat::client::DeduplicatedCombatCue;
use brawler::combat::{CombatCue, CombatEffectCue};
use serde::Deserialize;
use std::collections::HashMap;

mod flash;
mod numbers;
mod recovery;

pub(super) use flash::HitFlashMaterials;
use flash::{HitFlash, apply_hit_flash_materials, tick_hit_flashes};
use numbers::{FloatingNumberBook, ensure_floating_number_pool, project_floating_numbers};
use recovery::{HealthRecoveryLedger, collect_recovery_numbers};

const HIT_FEEDBACK_CATALOG: &str = include_str!("../../../../assets/catalogs/hit_feedback.ron");
const HIT_FEEDBACK_SCHEMA_VERSION: u16 = 1;
/// Hard ceiling on live floating numbers; the authored pool capacity may only be smaller.
pub(super) const MAX_FLOATING_NUMBERS: usize = 32;
const MAX_FLASH_MILLIS: f32 = 250.0;
const MAX_NUMBER_LIFETIME_MILLIS: f32 = 2_000.0;
const MAX_MERGE_MILLIS: f32 = 3_000.0;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct HitFeedbackCatalogSource {
    schema_version: u16,
    flash: FlashTuning,
    numbers: NumberTuning,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FlashTuning {
    pub(super) duration_millis: f32,
    pub(super) base_color_mix: f32,
    pub(super) emissive: f32,
    pub(super) reduced_duration_millis: f32,
    pub(super) reduced_base_color_mix: f32,
    pub(super) reduced_emissive: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct NumberTuning {
    pub(super) pool_capacity: usize,
    pub(super) lifetime_millis: f32,
    pub(super) pop_millis: f32,
    pub(super) pop_start_scale: f32,
    pub(super) pop_peak_scale: f32,
    pub(super) rise_pixels: f32,
    pub(super) fade_start_fraction: f32,
    pub(super) merge_window_millis: f32,
    pub(super) max_merge_millis: f32,
    pub(super) anchor_height_fraction: f32,
    pub(super) side_offset_pixels: f32,
    pub(super) stack_offset_pixels: f32,
    pub(super) styles: NumberStyles,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct NumberStyles {
    pub(super) dealt: NumberStyleTuning,
    pub(super) taken: NumberStyleTuning,
    pub(super) healing: NumberStyleTuning,
    pub(super) observed: NumberStyleTuning,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct NumberStyleTuning {
    pub(super) font_size: f32,
    pub(super) color: (f32, f32, f32),
}

impl NumberStyles {
    pub(super) const fn get(&self, style: HitStyle) -> NumberStyleTuning {
        match style {
            HitStyle::Dealt => self.dealt,
            HitStyle::Taken => self.taken,
            HitStyle::Healing => self.healing,
            HitStyle::Observed => self.observed,
        }
    }
}

/// Validated client-only hit feedback tuning, embedded from `assets/catalogs/hit_feedback.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(super) struct HitFeedbackCatalog {
    pub(super) flash: FlashTuning,
    pub(super) numbers: NumberTuning,
}

impl HitFeedbackCatalog {
    pub(super) fn embedded() -> Result<Self, String> {
        Self::from_ron(HIT_FEEDBACK_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: HitFeedbackCatalogSource = ron::from_str(source)
            .map_err(|error| format!("hit feedback catalog parse failed: {error}"))?;
        if source.schema_version != HIT_FEEDBACK_SCHEMA_VERSION {
            return Err(format!(
                "hit feedback catalog schema {} is not {HIT_FEEDBACK_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        let catalog = Self {
            flash: source.flash,
            numbers: source.numbers,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "the pool capacity is bounded by a 32-entry ceiling"
    )]
    fn validate(&self) -> Result<(), String> {
        let flash = self.flash;
        check_range(
            "flash.duration_millis",
            flash.duration_millis,
            16.0,
            MAX_FLASH_MILLIS,
        )?;
        check_range(
            "flash.reduced_duration_millis",
            flash.reduced_duration_millis,
            16.0,
            flash.duration_millis,
        )?;
        check_range("flash.base_color_mix", flash.base_color_mix, 0.0, 1.0)?;
        check_range("flash.emissive", flash.emissive, 0.0, 8.0)?;
        check_range(
            "flash.reduced_base_color_mix",
            flash.reduced_base_color_mix,
            0.0,
            flash.base_color_mix,
        )?;
        check_range(
            "flash.reduced_emissive",
            flash.reduced_emissive,
            0.0,
            flash.emissive,
        )?;
        let numbers = self.numbers;
        check_range(
            "numbers.pool_capacity",
            numbers.pool_capacity as f32,
            1.0,
            MAX_FLOATING_NUMBERS as f32,
        )?;
        check_range(
            "numbers.lifetime_millis",
            numbers.lifetime_millis,
            200.0,
            MAX_NUMBER_LIFETIME_MILLIS,
        )?;
        check_range(
            "numbers.pop_millis",
            numbers.pop_millis,
            0.0,
            numbers.lifetime_millis * 0.5,
        )?;
        check_range("numbers.pop_start_scale", numbers.pop_start_scale, 0.1, 1.0)?;
        check_range("numbers.pop_peak_scale", numbers.pop_peak_scale, 1.0, 2.0)?;
        check_range("numbers.rise_pixels", numbers.rise_pixels, 0.0, 120.0)?;
        check_range(
            "numbers.fade_start_fraction",
            numbers.fade_start_fraction,
            0.0,
            0.95,
        )?;
        check_range(
            "numbers.merge_window_millis",
            numbers.merge_window_millis,
            0.0,
            numbers.lifetime_millis,
        )?;
        check_range(
            "numbers.max_merge_millis",
            numbers.max_merge_millis,
            numbers.merge_window_millis,
            MAX_MERGE_MILLIS,
        )?;
        check_range(
            "numbers.anchor_height_fraction",
            numbers.anchor_height_fraction,
            0.0,
            1.5,
        )?;
        check_range(
            "numbers.side_offset_pixels",
            numbers.side_offset_pixels,
            -120.0,
            120.0,
        )?;
        check_range(
            "numbers.stack_offset_pixels",
            numbers.stack_offset_pixels,
            0.0,
            60.0,
        )?;
        for (name, style) in [
            ("dealt", numbers.styles.dealt),
            ("taken", numbers.styles.taken),
            ("healing", numbers.styles.healing),
            ("observed", numbers.styles.observed),
        ] {
            check_range(name, style.font_size, 8.0, 48.0)?;
            for channel in [style.color.0, style.color.1, style.color.2] {
                check_range(name, channel, 0.0, 1.0)?;
            }
        }
        Ok(())
    }
}

fn check_range(name: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "hit feedback catalog {name} = {value} is outside {min}..={max}"
        ))
    }
}

/// Who a floating number is about, from the local observer's point of view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum HitStyle {
    /// Damage the local player caused (weapon, ultimate, deployable, or initiated environment).
    Dealt,
    /// Damage the local player's fighter took.
    Taken,
    Healing,
    /// Damage between other combatants or objects.
    Observed,
}

/// One authoritative hit resolved to a presentation subject on this client.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HitFeedbackEvent {
    /// The replicated target entity on this client.
    pub(super) subject: Entity,
    pub(super) style: HitStyle,
    pub(super) amount: u16,
    /// Damage flashes the target; healing does not.
    pub(super) flash: bool,
    /// Authoritative cue position, used when the subject has no presentation root yet.
    pub(super) position: Vec2,
}

const fn damage_style(
    local: Option<NetworkEntityId>,
    source: Option<NetworkEntityId>,
    target: NetworkEntityId,
) -> HitStyle {
    match local {
        Some(local) if local.0 == target.0 => HitStyle::Taken,
        Some(local) if matches!(source, Some(source) if source.0 == local.0) => HitStyle::Dealt,
        _ => HitStyle::Observed,
    }
}

/// Maps one combat cue to hit feedback. A target that is not replicated to this observer (for
/// example a concealed enemy) resolves to `None`, so it can never leak a flash or number.
pub(super) fn combat_cue_hit_feedback(
    cue: &CombatCue,
    local: Option<NetworkEntityId>,
    resolve: impl Fn(NetworkEntityId) -> Option<Entity>,
) -> Option<HitFeedbackEvent> {
    let (target, style, amount, flash, position) = match cue {
        CombatCue::DamageApplied {
            source,
            target,
            position,
            amount,
            ..
        } => (
            *target,
            damage_style(local, source.initiating_fighter(), *target),
            *amount,
            true,
            *position,
        ),
        CombatCue::EffectApplied {
            target,
            position,
            effect: CombatEffectCue::Healing { amount, .. },
            ..
        } => (*target, HitStyle::Healing, *amount, false, *position),
        _ => return None,
    };
    if amount == 0 {
        return None;
    }
    Some(HitFeedbackEvent {
        subject: resolve(target)?,
        style,
        amount,
        flash,
        position: position.as_vec2(),
    })
}

/// Maps damage to a damageable world object or Vault Break idol to hit feedback. The object must be
/// replicated to this observer.
pub(super) fn object_damage_hit_feedback(
    target: brawler::map::DamageableTargetIdentity,
    source_subject: Option<NetworkEntityId>,
    amount: u16,
    position: brawler::combat::WorldPoint,
    local: Option<NetworkEntityId>,
    resolve: impl Fn(brawler::map::DamageableTargetIdentity) -> Option<Entity>,
) -> Option<HitFeedbackEvent> {
    if amount == 0 {
        return None;
    }
    let style = match (local, source_subject) {
        (Some(local), Some(source)) if local.0 == source.0 => HitStyle::Dealt,
        _ => HitStyle::Observed,
    };
    Some(HitFeedbackEvent {
        subject: resolve(target)?,
        style,
        amount,
        flash: true,
        position: position.as_vec2(),
    })
}

type CombatHitRoots<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static CombatVisualOwner),
    Or<(With<V3FighterVisual>, With<combat::SentryVisual3d>)>,
>;

/// Presentation roots that can carry a hit flash and anchor a floating number.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct HitFeedbackRoots<'w, 's> {
    combat: CombatHitRoots<'w, 's>,
    zombies: Query<'w, 's, (Entity, &'static zombie::ZombieVisual)>,
    map_objects: Query<'w, 's, (Entity, &'static DynamicMapVisual)>,
    vault_break_safes: Query<'w, 's, (Entity, &'static VaultBreakSafeVisual)>,
    identities: Query<'w, 's, &'static brawler::map::DamageableTargetIdentity>,
    transforms: Query<'w, 's, &'static GlobalTransform>,
}

impl HitFeedbackRoots<'_, '_> {
    /// Every presentation root of a replicated subject (normally exactly one).
    fn roots(&self, subject: Entity) -> Vec<Entity> {
        let mut roots = Vec::new();
        roots.extend(
            self.combat
                .iter()
                .filter_map(|(root, owner)| (owner.0 == subject).then_some(root)),
        );
        roots.extend(
            self.zombies
                .iter()
                .filter_map(|(root, visual)| (visual.owner == subject).then_some(root)),
        );
        roots.extend(
            self.vault_break_safes
                .iter()
                .filter_map(|(root, visual)| (visual.owner == subject).then_some(root)),
        );
        if let Ok(brawler::map::DamageableTargetIdentity::MapObject {
            generation,
            placement_id,
        }) = self.identities.get(subject)
        {
            roots.extend(self.map_objects.iter().filter_map(|(root, visual)| {
                (visual.placement_id == *placement_id && visual.generation == generation.generation)
                    .then_some(root)
            }));
        }
        roots
    }

    fn anchor(&self, roots: &[Entity], fallback: Vec2) -> Vec3 {
        roots
            .first()
            .and_then(|root| self.transforms.get(*root).ok())
            .map_or_else(|| ground_position(fallback), GlobalTransform::translation)
    }
}

type ReplicatedTargets<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static NetworkEntityId>,
        Option<&'static brawler::map::DamageableTargetIdentity>,
    ),
    Or<(
        With<NetworkEntityId>,
        With<brawler::map::DamageableTargetIdentity>,
    )>,
>;

/// Turn this frame's authoritative hit cues into flashes and floating numbers.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the system reads the three authoritative cue streams that feed one presentation channel"
)]
pub(super) fn collect_hit_feedback(
    mut commands: Commands,
    catalog: Res<HitFeedbackCatalog>,
    settings: Option<Res<ClientShellSettings>>,
    mut book: ResMut<FloatingNumberBook>,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut vault_break_cues: MessageReader<brawler::matchplay::ReceivedVaultBreakObjectiveCue>,
    world_objects: Option<Res<brawler::map::ReceivedWorldObjectCues>>,
    mut seen_world_objects: Local<std::collections::VecDeque<brawler::combat::CombatEventId>>,
    local: Query<&NetworkEntityId, (With<Fighter>, With<Controlled>)>,
    targets: ReplicatedTargets,
    roots: HitFeedbackRoots,
) {
    let local = local.iter().next().copied();
    let mut by_network_id: Option<HashMap<u64, Entity>> = None;
    let mut events = Vec::new();
    for DeduplicatedCombatCue(cue) in cues.read() {
        let index = by_network_id.get_or_insert_with(|| {
            targets
                .iter()
                .filter_map(|(entity, id, _)| id.map(|id| (id.0, entity)))
                .collect()
        });
        if let Some(event) = combat_cue_hit_feedback(cue, local, |id| index.get(&id.0).copied()) {
            events.push(event);
        }
    }
    let resolve_object = |identity: brawler::map::DamageableTargetIdentity| {
        targets
            .iter()
            .find_map(|(entity, _, candidate)| (candidate == Some(&identity)).then_some(entity))
    };
    // The world-object VFX producer drains this inbox after this system each frame; remembering
    // seen ids keeps the channel exact if a cue survives into another frame.
    if let Some(world_objects) = world_objects {
        for cue in &world_objects.0 {
            let brawler::map::WorldObjectCue::Damaged {
                event_id,
                source_subject,
                target,
                position,
                amount,
                ..
            } = *cue
            else {
                continue;
            };
            if seen_world_objects.contains(&event_id) {
                continue;
            }
            if seen_world_objects.len() >= brawler::map::MAX_WORLD_OBJECT_CUES {
                seen_world_objects.pop_front();
            }
            seen_world_objects.push_back(event_id);
            events.extend(object_damage_hit_feedback(
                target,
                source_subject,
                amount,
                position,
                local,
                resolve_object,
            ));
        }
    }
    for brawler::matchplay::ReceivedVaultBreakObjectiveCue(cue) in vault_break_cues.read() {
        events.extend(object_damage_hit_feedback(
            cue.target,
            cue.source_subject,
            cue.amount,
            cue.position,
            local,
            resolve_object,
        ));
    }
    let reduced = settings.is_some_and(|settings| settings.reduced_combat_effects);
    for event in events {
        let subject_roots = roots.roots(event.subject);
        if event.flash {
            for root in &subject_roots {
                commands
                    .entity(*root)
                    .try_insert(HitFlash::new(&catalog.flash, reduced));
            }
        }
        let anchor = roots.anchor(&subject_roots, event.position);
        book.push(
            event.subject,
            event.style,
            event.amount,
            anchor,
            &catalog.numbers,
        );
    }
}

/// Registers hit feedback into the world presentation phases.
pub(super) fn install(app: &mut App) {
    let catalog =
        HitFeedbackCatalog::embedded().expect("the embedded hit feedback catalog must be valid");
    app.insert_resource(catalog)
        .insert_resource(FloatingNumberBook::with_capacity(
            catalog.numbers.pool_capacity,
        ))
        .init_resource::<HitFlashMaterials<StandardMaterial>>()
        .init_resource::<HitFlashMaterials<ToonMaterial>>()
        .init_resource::<HealthRecoveryLedger>()
        .add_message::<DeduplicatedCombatCue>()
        .add_message::<brawler::matchplay::ReceivedVaultBreakObjectiveCue>()
        .add_systems(
            Update,
            (
                ensure_floating_number_pool.in_set(WorldPresentationSet::ReconcileState),
                // Reads the world-object inbox before the VFX producer drains it.
                collect_hit_feedback
                    .before(vfx::VfxRequestSet)
                    .in_set(WorldPresentationSet::ConsumeCues),
                // Pickup and replicated-health recovery numbers; reads this frame's pickup inbox.
                collect_recovery_numbers
                    .after(collect_hit_feedback)
                    .after(brawler::map::ClientPickupCueSet)
                    .in_set(WorldPresentationSet::ConsumeCues),
                (
                    apply_hit_flash_materials::<StandardMaterial>,
                    apply_hit_flash_materials::<ToonMaterial>,
                    tick_hit_flashes,
                )
                    .chain()
                    .after(combat::update_fighter_concealment_visuals::<StandardMaterial>)
                    .after(combat::update_fighter_concealment_visuals::<ToonMaterial>)
                    .after(update_damageable_map_visuals)
                    .after(update_vault_break_safe_status_visuals)
                    .in_set(WorldPresentationSet::Animate),
            ),
        )
        .add_systems(
            PostUpdate,
            project_floating_numbers
                .after(TransformSystems::Propagate)
                .after(CameraUpdateSystems)
                .before(UiSystems::Prepare),
        );
}

#[cfg(test)]
mod tests;
