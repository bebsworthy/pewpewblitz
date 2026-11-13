//! Pooled, bounded floating damage and healing numbers.
//!
//! [`FloatingNumberBook`] is the pure bookkeeping (merge, stack, evict, age); a fixed set of UI
//! text slots spawned once mirrors it. Numbers are world-anchored where the hit landed and
//! projected through the arena camera after it updates each frame.

use super::*;

const FIGHTER_BODY_WORLD_HEIGHT: f32 = FIGHTER_VISUAL_HEIGHT_WORLD;
/// Deterministic lateral pattern, in stack-offset units, for numbers that cannot merge.
const LATERAL_PATTERN: [f32; 5] = [0.0, 1.0, -1.0, 2.0, -2.0];
/// Above the overhead plates (120-123) so a number is never hidden behind a plate.
const FLOATING_NUMBER_Z_INDEX: i32 = 124;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::client::presentation_3d) struct FloatingNumber {
    pub(super) subject: Entity,
    pub(super) style: HitStyle,
    pub(super) amount: u32,
    /// World point (ground under the subject) the number floats from.
    pub(super) anchor: Vec3,
    /// Lateral stack slot in stack-offset units.
    pub(super) lateral: f32,
    /// Seconds since the number last popped (spawn or merge).
    pub(super) age: f32,
    /// Seconds since the number first appeared.
    pub(super) merge_age: f32,
    serial: u64,
}

/// What a push did to the pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client::presentation_3d) enum FloatingNumberPush {
    Merged(usize),
    Started(usize),
    /// The pool was full and the oldest number was recycled.
    Recycled(usize),
}

/// Bounded live floating numbers. The slot count is fixed at construction.
#[derive(Resource, Debug)]
pub(in crate::client::presentation_3d) struct FloatingNumberBook {
    entries: Vec<Option<FloatingNumber>>,
    next_serial: u64,
}

impl FloatingNumberBook {
    pub(in crate::client::presentation_3d) fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: vec![None; capacity.clamp(1, MAX_FLOATING_NUMBERS)],
            next_serial: 0,
        }
    }

    pub(super) fn capacity(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn live(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub(super) fn get(&self, slot: usize) -> Option<&FloatingNumber> {
        self.entries.get(slot)?.as_ref()
    }

    /// Record one authoritative hit. Rapid hits on one subject with one style sum into one
    /// number; otherwise a new number starts beside the subject's other live numbers, recycling
    /// the oldest number when the pool is full.
    pub(super) fn push(
        &mut self,
        subject: Entity,
        style: HitStyle,
        amount: u16,
        anchor: Vec3,
        tuning: &NumberTuning,
    ) -> FloatingNumberPush {
        let merge_window = tuning.merge_window_millis / 1_000.0;
        let max_merge = tuning.max_merge_millis / 1_000.0;
        if let Some((slot, entry)) = self
            .entries
            .iter_mut()
            .enumerate()
            .filter_map(|(slot, entry)| entry.as_mut().map(|entry| (slot, entry)))
            .filter(|(_, entry)| {
                entry.subject == subject
                    && entry.style == style
                    && entry.age <= merge_window
                    && entry.merge_age <= max_merge
            })
            .max_by_key(|(_, entry)| entry.serial)
        {
            entry.amount = entry.amount.saturating_add(u32::from(amount));
            entry.anchor = anchor;
            entry.age = 0.0;
            return FloatingNumberPush::Merged(slot);
        }
        let stacked = self
            .entries
            .iter()
            .flatten()
            .filter(|entry| entry.subject == subject)
            .count();
        let entry = FloatingNumber {
            subject,
            style,
            amount: u32::from(amount),
            anchor,
            lateral: LATERAL_PATTERN[stacked % LATERAL_PATTERN.len()],
            age: 0.0,
            merge_age: 0.0,
            serial: self.next_serial,
        };
        self.next_serial = self.next_serial.wrapping_add(1);
        if let Some(slot) = self.entries.iter().position(Option::is_none) {
            self.entries[slot] = Some(entry);
            return FloatingNumberPush::Started(slot);
        }
        let slot = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| entry.map(|entry| (slot, entry.serial)))
            .min_by_key(|(_, serial)| *serial)
            .map_or(0, |(slot, _)| slot);
        self.entries[slot] = Some(entry);
        FloatingNumberPush::Recycled(slot)
    }

    /// Age every live number and free the expired ones.
    pub(super) fn advance(&mut self, seconds: f32, tuning: &NumberTuning) {
        let lifetime = tuning.lifetime_millis / 1_000.0;
        for slot in &mut self.entries {
            if let Some(entry) = slot.as_mut() {
                entry.age += seconds;
                entry.merge_age += seconds;
                if entry.age >= lifetime {
                    *slot = None;
                }
            }
        }
    }
}

/// Scale, rise (pixels), and opacity of a number `age` seconds after it popped.
pub(super) fn number_animation(
    age: f32,
    tuning: &NumberTuning,
    reduced_motion: bool,
) -> (f32, f32, f32) {
    let lifetime = tuning.lifetime_millis / 1_000.0;
    let progress = (age / lifetime).clamp(0.0, 1.0);
    let fade_start = tuning.fade_start_fraction;
    let alpha = if progress <= fade_start {
        1.0
    } else {
        1.0 - (progress - fade_start) / (1.0 - fade_start)
    };
    if reduced_motion {
        return (1.0, 0.0, alpha);
    }
    let pop = tuning.pop_millis / 1_000.0;
    let scale = if pop <= 0.0 || age >= pop {
        1.0
    } else {
        // Overshoot to the peak over the first 40% of the pop, then settle to 1.0.
        let t = age / pop;
        if t < 0.4 {
            let rise = t / 0.4;
            tuning.pop_start_scale + (tuning.pop_peak_scale - tuning.pop_start_scale) * rise
        } else {
            let settle = (t - 0.4) / 0.6;
            tuning.pop_peak_scale + (1.0 - tuning.pop_peak_scale) * settle
        }
    };
    // Ease-out rise.
    let rise = tuning.rise_pixels * (1.0 - (1.0 - progress) * (1.0 - progress));
    (scale, rise, alpha)
}

fn number_label(style: HitStyle, amount: u32) -> String {
    match style {
        HitStyle::Healing => format!("+{amount}"),
        HitStyle::Dealt | HitStyle::Taken | HitStyle::Observed => amount.to_string(),
    }
}

/// One pooled UI text slot mirroring [`FloatingNumberBook`] entry `index`.
#[derive(Component, Debug)]
pub(in crate::client::presentation_3d) struct FloatingNumberSlot {
    pub(super) index: usize,
    shown: Option<(HitStyle, u32)>,
}

/// Spawn the fixed pool of hidden number slots once (and again only if something despawned them).
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource system parameters by value"
)]
pub(in crate::client::presentation_3d) fn ensure_floating_number_pool(
    mut commands: Commands,
    book: Res<FloatingNumberBook>,
    asset_server: Option<Res<AssetServer>>,
    slots: Query<&FloatingNumberSlot>,
) {
    let mut present = vec![false; book.capacity()];
    for slot in &slots {
        if let Some(flag) = present.get_mut(slot.index) {
            *flag = true;
        }
    }
    if present.iter().all(|flag| *flag) {
        return;
    }
    let font = ui::theme::display_font_handle(asset_server.as_deref());
    for (index, _) in present.iter().enumerate().filter(|(_, flag)| !**flag) {
        commands.spawn((
            FloatingNumberSlot { index, shown: None },
            Text::new(""),
            ui::theme::hud_text(&font, 24.0),
            TextColor(Color::WHITE),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTransform::IDENTITY,
            GlobalZIndex(FLOATING_NUMBER_Z_INDEX),
            Visibility::Hidden,
            Name::new("floating hit number"),
        ));
    }
}

type FloatingNumberSlotQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut FloatingNumberSlot,
        &'static mut Node,
        &'static mut Text,
        &'static mut TextFont,
        &'static mut TextColor,
        &'static mut TextShadow,
        &'static mut UiTransform,
        &'static mut Visibility,
    ),
>;

/// Age the pool and place each live number in the viewport for this frame.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource system parameters by value"
)]
pub(in crate::client::presentation_3d) fn project_floating_numbers(
    time: Res<Time>,
    catalog: Res<HitFeedbackCatalog>,
    settings: Option<Res<ClientShellSettings>>,
    mut book: ResMut<FloatingNumberBook>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    mut slots: FloatingNumberSlotQuery,
) {
    let tuning = catalog.numbers;
    book.advance(time.delta_secs(), &tuning);
    let reduced_motion = settings.is_some_and(|settings| settings.reduced_motion);
    let camera = cameras.single().ok();
    for (
        mut slot,
        mut node,
        mut text,
        mut font,
        mut color,
        mut shadow,
        mut transform,
        mut visibility,
    ) in &mut slots
    {
        let Some(entry) = book.get(slot.index).copied() else {
            visibility.set_if_neq(Visibility::Hidden);
            if slot.shown.is_some() {
                slot.shown = None;
            }
            continue;
        };
        let style = tuning.styles.get(entry.style);
        let world =
            entry.anchor + Vec3::Y * FIGHTER_BODY_WORLD_HEIGHT * tuning.anchor_height_fraction;
        let Some(viewport) = camera
            .and_then(|(camera, camera_transform)| {
                camera.world_to_viewport(camera_transform, world).ok()
            })
            .filter(|point| point.is_finite())
        else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        if slot.shown != Some((entry.style, entry.amount)) {
            text.0 = number_label(entry.style, entry.amount);
            font.font_size = FontSize::Px(style.font_size);
            shadow.offset = Vec2::splat((style.font_size * 0.07).max(1.5));
            slot.shown = Some((entry.style, entry.amount));
        }
        let (scale, rise, alpha) = number_animation(entry.age, &tuning, reduced_motion);
        let (red, green, blue) = style.color;
        color.0 = Color::srgba(red, green, blue, alpha);
        shadow.color = ui::theme::INK.with_alpha(alpha);
        node.left =
            px(viewport.x + tuning.side_offset_pixels + entry.lateral * tuning.stack_offset_pixels);
        node.top = px(viewport.y - rise - style.font_size * 0.6);
        transform.scale = Vec2::splat(scale);
        visibility.set_if_neq(Visibility::Inherited);
    }
}
