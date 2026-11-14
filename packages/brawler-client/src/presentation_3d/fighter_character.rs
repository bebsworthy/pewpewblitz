//! Riff, the production fighter model (BRL-0154).
//!
//! Owns the imported character's asset contract (scale, facing, weapon socket, clip names), the
//! weapon-family hold/attack clips, the pure priority that maps replicated state and cues to one
//! motion, and a presentation-only squash-and-stretch layer. Nothing here feeds simulation: the
//! visual height is independent of the authoritative radius, and squash only scales the imported
//! scene root below the fighter's visual root.
//!
//! The GLB is a derivative of `asset_src/characters/riff/build_riff.py`; regenerate it with
//! `just riff-asset` rather than editing it.

#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;
use brawler::combat::client::DeduplicatedCombatCue;
use brawler::combat::{CombatCue, DeliveryMethod};

pub(crate) const RIFF_ASSET_PATH: &str = "brawler/models/original/characters/riff/chr_riff.glb";
/// Riff faces glTF +Z, while Brawler fighter roots face local +X.
pub(crate) const RIFF_FORWARD_CORRECTION: f32 = core::f32::consts::FRAC_PI_2;
/// Reviewed export bounds (build report): sole bottom and ear tip in source units.
const RIFF_SOURCE_GROUND: f32 = 0.04;
const RIFF_SOURCE_TOP: f32 = 2.506;
/// Visual height including the ears: about 1.6 map cells (docs/11). Independent of collision.
pub(crate) const RIFF_VISUAL_HEIGHT_WORLD: f32 = brawler::map::MAP_CELL_SIZE_WORLD * 1.6;
const RIFF_GAMEPLAY_SCALE: f32 = RIFF_VISUAL_HEIGHT_WORLD / (RIFF_SOURCE_TOP - RIFF_SOURCE_GROUND);
/// Dashboard and brawler-screen preview scale: the same framing height the Kenney preview had.
pub(crate) const DASHBOARD_RIFF_SCALE: f32 = 32.0;
/// The shared weapon attaches to this non-deforming socket node.
const RIFF_WEAPON_SOCKET: &str = "socket_weapon_r";
/// The exported socket's rest frame is a half turn about (0, 1, 1)/√2 from the scene frame; the
/// same half turn maps the blaster's +Z barrel and +Y top onto Riff's forward and up.
const RIFF_BLASTER_GRIP: Quat = Quat::from_xyzw(
    0.0,
    core::f32::consts::FRAC_1_SQRT_2,
    core::f32::consts::FRAC_1_SQRT_2,
    0.0,
);
/// Blaster scale inside Riff's source units: about 1.5x a plausible pistol for his hand.
const RIFF_BLASTER_SCALE: f32 = 0.78;
/// Run clip footfalls are authored at phase 0 and 0.5 of the loop.
const RUN_FOOTFALL_PHASES: [f32; 2] = [0.0, 0.5];
const RUN_FOOTFALL_WIDTH: f32 = 0.12;
const RUN_FOOTFALL_SQUASH: f32 = 0.05;
const FIRE_SQUASH: f32 = 0.07;
const FIRE_SQUASH_SECONDS: f32 = 0.14;
const LANDING_SQUASH: f32 = 0.14;
const LANDING_SQUASH_SECONDS: f32 = 0.22;
const MOTION_BLEND: Duration = Duration::from_millis(100);

/// Clip names exported by the builder, in graph order.
const CLIP_NAMES: [&str; 15] = [
    "idle",
    "run",
    "dash",
    "hit_front",
    "ko",
    "victory",
    "ultimate",
    "combat_idle_1h",
    "combat_idle_heavy",
    "combat_idle_throwable",
    "combat_idle_melee",
    "attack_1h",
    "attack_heavy",
    "attack_throwable",
    "attack_melee",
];

/// Animation family of the primary weapon, from its replicated delivery method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WeaponFamily {
    OneHanded,
    Heavy,
    Throwable,
    Melee,
}

impl WeaponFamily {
    pub(crate) const fn of(delivery: &DeliveryMethod) -> Self {
        match delivery {
            DeliveryMethod::Straight { .. } | DeliveryMethod::StickyStraight { .. } => {
                Self::OneHanded
            }
            DeliveryMethod::ConeSpray { .. } => Self::Heavy,
            DeliveryMethod::Lobbed { .. } | DeliveryMethod::Splash { .. } => Self::Throwable,
            DeliveryMethod::MeleeArc { .. } => Self::Melee,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::OneHanded => 0,
            Self::Heavy => 1,
            Self::Throwable => 2,
            Self::Melee => 3,
        }
    }
}

/// Graph nodes and one-shot durations of Riff's clip set.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FighterClips {
    pub(crate) idle: AnimationNodeIndex,
    run: AnimationNodeIndex,
    dash: AnimationNodeIndex,
    hit: AnimationNodeIndex,
    defeat: AnimationNodeIndex,
    victory: AnimationNodeIndex,
    ultimate: AnimationNodeIndex,
    hold: [AnimationNodeIndex; 4],
    attack: [AnimationNodeIndex; 4],
    hit_seconds: f32,
    ultimate_seconds: f32,
    run_seconds: f32,
    attack_seconds: [f32; 4],
}

impl FighterClips {
    /// Builds the graph from the exported clip contract, or `None` (primitive fallback) when any
    /// clip is missing.
    pub(crate) fn from_gltf(
        gltf: &Gltf,
        clips: &Assets<AnimationClip>,
    ) -> Option<(AnimationGraph, Self)> {
        let handles = CLIP_NAMES
            .iter()
            .map(|name| gltf.named_animations.get(*name).cloned())
            .collect::<Option<Vec<_>>>()?;
        let durations = handles
            .iter()
            .map(|handle| clips.get(handle).map(AnimationClip::duration))
            .collect::<Option<Vec<_>>>()?;
        let (graph, nodes) = AnimationGraph::from_clips(handles);
        let node = |index: usize| nodes[index];
        Some((
            graph,
            Self {
                idle: node(0),
                run: node(1),
                dash: node(2),
                hit: node(3),
                defeat: node(4),
                victory: node(5),
                ultimate: node(6),
                hold: [node(7), node(8), node(9), node(10)],
                attack: [node(11), node(12), node(13), node(14)],
                hit_seconds: durations[3],
                ultimate_seconds: durations[6],
                run_seconds: durations[1],
                attack_seconds: [durations[11], durations[12], durations[13], durations[14]],
            },
        ))
    }

    fn node(&self, motion: CharacterMotion) -> AnimationNodeIndex {
        match motion {
            CharacterMotion::Idle => self.idle,
            CharacterMotion::Hold(family) => self.hold[family.index()],
            CharacterMotion::Run => self.run,
            CharacterMotion::Attack(family) => self.attack[family.index()],
            CharacterMotion::Hit => self.hit,
            CharacterMotion::Dash => self.dash,
            CharacterMotion::Ultimate => self.ultimate,
            CharacterMotion::Victory => self.victory,
            CharacterMotion::Defeated => self.defeat,
        }
    }
}

/// Root transform of an imported Riff scene: facing correction, uniform scale, grounded soles.
pub(crate) fn riff_scene_transform(yaw_offset: f32, scale: f32) -> Transform {
    Transform {
        translation: Vec3::Y * -RIFF_SOURCE_GROUND * scale,
        rotation: Quat::from_rotation_y(RIFF_FORWARD_CORRECTION + yaw_offset),
        scale: Vec3::splat(scale),
    }
}

pub(super) fn gameplay_riff_transform() -> Transform {
    riff_scene_transform(0.0, RIFF_GAMEPLAY_SCALE)
}

/// The shared blaster, posed for a child of Riff's weapon socket.
pub(crate) fn blaster_attachment(blaster_scene: Handle<WorldAsset>) -> impl Bundle {
    (
        WorldAssetRoot(blaster_scene),
        Transform {
            rotation: RIFF_BLASTER_GRIP,
            scale: Vec3::splat(RIFF_BLASTER_SCALE),
            ..default()
        },
        Name::new("Riff attached blaster-a"),
    )
}

/// Finds the weapon socket and the animation player inside a ready Riff scene.
pub(crate) fn find_socket_and_player(
    descendants: &[Entity],
    names: &Query<&Name>,
    players: &Query<&mut AnimationPlayer>,
) -> Option<(Entity, Entity)> {
    let socket = descendants.iter().copied().find(|entity| {
        names
            .get(*entity)
            .is_ok_and(|name| name.as_str() == RIFF_WEAPON_SOCKET)
    })?;
    let player = descendants
        .iter()
        .copied()
        .find(|entity| players.get(*entity).is_ok())?;
    Some((socket, player))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CharacterMotion {
    Idle,
    Hold(WeaponFamily),
    Run,
    Attack(WeaponFamily),
    Hit,
    Dash,
    Ultimate,
    Victory,
    Defeated,
}

impl CharacterMotion {
    const fn loops(self) -> bool {
        !matches!(
            self,
            Self::Attack(_) | Self::Hit | Self::Ultimate | Self::Defeated
        )
    }
}

/// Replicated facts and cue windows one fighter's motion is chosen from.
#[allow(
    clippy::struct_excessive_bools,
    reason = "a flat snapshot of independent facts keeps the priority function obvious"
)]
#[derive(Clone, Copy, Debug)]
struct MotionInputs {
    defeated: bool,
    victorious: bool,
    dashing: bool,
    ultimate: bool,
    attacking: bool,
    hit: bool,
    moving: bool,
    family: WeaponFamily,
}

/// Highest-priority motion first: a defeated fighter never celebrates, a dash overrides firing,
/// and firing reads over a hit flinch so attacks stay legible.
const fn select_motion(inputs: MotionInputs) -> CharacterMotion {
    if inputs.defeated {
        CharacterMotion::Defeated
    } else if inputs.victorious {
        CharacterMotion::Victory
    } else if inputs.dashing {
        CharacterMotion::Dash
    } else if inputs.ultimate {
        CharacterMotion::Ultimate
    } else if inputs.attacking {
        CharacterMotion::Attack(inputs.family)
    } else if inputs.hit {
        CharacterMotion::Hit
    } else if inputs.moving {
        CharacterMotion::Run
    } else {
        CharacterMotion::Hold(inputs.family)
    }
}

pub(super) fn character_is_visually_defeated(
    current_health: u16,
    has_defeated_marker: bool,
) -> bool {
    has_defeated_marker && current_health == 0
}

fn team_won(phase: MatchPhase, team: brawler::combat::TeamId) -> bool {
    match phase {
        MatchPhase::Completed { result, .. } => match result {
            brawler::matchplay::MatchResult::TeamVictory { team: winner }
            | brawler::matchplay::MatchResult::Forfeit { winner, .. } => winner == team,
            brawler::matchplay::MatchResult::Draw
            | brawler::matchplay::MatchResult::Cleared
            | brawler::matchplay::MatchResult::Overrun => false,
        },
        _ => false,
    }
}

/// One bounded squash impulse: a vertical squash that rebounds into a slight stretch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SquashImpulse {
    amplitude: f32,
    duration: f32,
    elapsed: f32,
}

impl SquashImpulse {
    const fn new(amplitude: f32, duration: f32) -> Self {
        Self {
            amplitude,
            duration,
            elapsed: 0.0,
        }
    }

    /// Signed vertical squash (positive compresses) at the current time.
    fn value(self) -> f32 {
        if self.duration <= 0.0 || self.elapsed >= self.duration {
            return 0.0;
        }
        let progress = self.elapsed / self.duration;
        self.amplitude * (core::f32::consts::PI * 1.5 * progress).cos() * (1.0 - progress)
    }
}

/// Squash at a run footfall for the given loop phase in `[0, 1)`.
fn footfall_squash(phase: f32) -> f32 {
    RUN_FOOTFALL_PHASES
        .iter()
        .map(|contact| {
            let distance = (phase - contact).abs();
            let wrapped = distance.min(1.0 - distance);
            (1.0 - wrapped / RUN_FOOTFALL_WIDTH).max(0.0)
        })
        .fold(0.0, f32::max)
        * RUN_FOOTFALL_SQUASH
}

/// Volume-preserving scale for a vertical squash (positive) or stretch (negative).
fn squash_scale(squash: f32) -> Vec3 {
    let vertical = (1.0 - squash).clamp(0.7, 1.3);
    let horizontal = vertical.recip().sqrt();
    Vec3::new(horizontal, vertical, horizontal)
}

#[derive(Component)]
pub(super) struct V3CharacterRuntime {
    owner: Entity,
    visual_root: Entity,
    player: Entity,
    current: CharacterMotion,
    bind_pose: Vec<(Entity, Transform)>,
    base_scale: Vec3,
    last_shoot_seconds: f32,
    attack_seconds: f32,
    hit_seconds: f32,
    ultimate_seconds: f32,
    ability_ready: bool,
    dashing: bool,
    impulse: SquashImpulse,
}

#[derive(Component)]
pub(super) struct V3CharacterScene {
    pub(super) owner: Entity,
    pub(super) visual_root: Entity,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the scene-ready observer validates and installs one bounded imported hierarchy"
)]
pub(super) fn setup_imported_character(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    imported: Res<Imported3dAssets>,
    scene_roots: Query<&V3CharacterScene>,
    children: Query<&Children>,
    names: Query<&Name>,
    transforms: Query<&Transform>,
    mut players: Query<&mut AnimationPlayer>,
    mut fallbacks: Query<(&V3FallbackVisual, &mut Visibility)>,
) {
    let Ok(scene) = scene_roots.get(ready.entity) else {
        return;
    };
    let descendants = children.iter_descendants(ready.entity).collect::<Vec<_>>();
    let Some((socket, player)) = find_socket_and_player(&descendants, &names, &players) else {
        warn!(owner = ?scene.owner, "imported fighter hierarchy misses its weapon socket or AnimationPlayer; retaining fallback");
        return;
    };
    let bind_pose = descendants
        .iter()
        .filter_map(|entity| {
            transforms
                .get(*entity)
                .ok()
                .map(|transform| (*entity, *transform))
        })
        .collect();
    commands.entity(socket).with_child((
        ToonScene,
        blaster_attachment(imported.blaster_scene.clone()),
    ));
    if let Ok(mut animation_player) = players.get_mut(player) {
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut animation_player, imported.clips.idle, Duration::ZERO)
            .repeat();
        commands
            .entity(player)
            .insert(AnimationGraphHandle(imported.animation_graph.clone()))
            .insert(transitions);
    }
    let base_scale = transforms
        .get(ready.entity)
        .map_or(Vec3::ONE, |transform| transform.scale);
    commands.entity(ready.entity).insert(V3CharacterRuntime {
        owner: scene.owner,
        visual_root: scene.visual_root,
        player,
        current: CharacterMotion::Idle,
        bind_pose,
        base_scale,
        last_shoot_seconds: 0.0,
        attack_seconds: 0.0,
        hit_seconds: 0.0,
        ultimate_seconds: 0.0,
        ability_ready: false,
        dashing: false,
        impulse: SquashImpulse::default(),
    });
    for (fallback, mut visibility) in &mut fallbacks {
        if fallback.owner == scene.owner {
            *visibility = Visibility::Hidden;
        }
    }
}

type CharacterOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static brawler::combat::CurrentHealth,
        Option<&'static brawler::combat::Defeated>,
        &'static brawler::combat::TeamId,
        Option<&'static brawler::builds::ResolvedMatchLoadout>,
        Option<&'static brawler::builds::AbilityState>,
    ),
    With<Fighter>,
>;

/// Per-frame replicated facts about one fighter that drive its cue windows.
#[allow(
    clippy::struct_excessive_bools,
    reason = "a flat snapshot of independent replicated facts"
)]
#[derive(Clone, Copy, Debug)]
struct FighterFacts {
    family: WeaponFamily,
    defeated: bool,
    victorious: bool,
    moving: bool,
    hit: bool,
    shoot_seconds: f32,
    ability: Option<brawler::builds::AbilityPhase>,
}

/// One-frame edges detected while advancing the cue windows.
#[derive(Clone, Copy, Debug, Default)]
struct WindowEdges {
    fired: bool,
    landed: bool,
}

impl V3CharacterRuntime {
    /// Advances the attack, hit, and ultimate windows and records dash and ability edges.
    fn advance_windows(
        &mut self,
        clips: &FighterClips,
        facts: FighterFacts,
        delta: f32,
    ) -> WindowEdges {
        let fired = facts.shoot_seconds > self.last_shoot_seconds;
        self.last_shoot_seconds = facts.shoot_seconds;
        self.attack_seconds = if fired {
            clips.attack_seconds[facts.family.index()]
        } else {
            (self.attack_seconds - delta).max(0.0)
        };
        self.hit_seconds = if facts.hit {
            clips.hit_seconds
        } else {
            (self.hit_seconds - delta).max(0.0)
        };
        let dashing = matches!(
            facts.ability,
            Some(brawler::builds::AbilityPhase::Dashing { .. })
        );
        let ready = matches!(facts.ability, Some(brawler::builds::AbilityPhase::Ready));
        // A Ready-to-active transition is an activation; a reset while defeated is not.
        let activated = self.ability_ready
            && !ready
            && !dashing
            && !facts.defeated
            && self.current != CharacterMotion::Defeated;
        self.ultimate_seconds = if activated {
            clips.ultimate_seconds
        } else {
            (self.ultimate_seconds - delta).max(0.0)
        };
        self.ability_ready = ready;
        let landed = self.dashing && !dashing;
        self.dashing = dashing;
        WindowEdges { fired, landed }
    }

    fn next_motion(&self, facts: FighterFacts) -> CharacterMotion {
        select_motion(MotionInputs {
            defeated: facts.defeated,
            victorious: facts.victorious,
            dashing: self.dashing,
            ultimate: self.ultimate_seconds > 0.0,
            attacking: self.attack_seconds > 0.0,
            hit: self.hit_seconds > 0.0,
            moving: facts.moving,
            family: facts.family,
        })
    }

    /// Starts a squash for firing, or a stronger one for landing from a dash or a respawn.
    fn start_impulses(&mut self, edges: WindowEdges, respawned: bool, delta: f32) {
        if edges.fired {
            self.impulse = SquashImpulse::new(FIRE_SQUASH, FIRE_SQUASH_SECONDS);
        }
        if edges.landed || respawned {
            self.impulse = SquashImpulse::new(LANDING_SQUASH, LANDING_SQUASH_SECONDS);
        }
        self.impulse.elapsed += delta;
    }

    fn squashed_scale(&self, reduced_motion: bool, run_phase: Option<f32>) -> Vec3 {
        let squash = if reduced_motion || self.current == CharacterMotion::Defeated {
            0.0
        } else {
            self.impulse.value() + run_phase.map_or(0.0, footfall_squash)
        };
        self.base_scale * squash_scale(squash)
    }
}

fn damaged_fighters(
    cues: &mut MessageReader<DeduplicatedCombatCue>,
    network_ids: &Query<(Entity, &NetworkEntityId), With<Fighter>>,
) -> Vec<Entity> {
    cues.read()
        .filter_map(|DeduplicatedCombatCue(cue)| match cue {
            CombatCue::DamageApplied { target, .. } => Some(*target),
            _ => None,
        })
        .filter_map(|target| {
            network_ids
                .iter()
                .find_map(|(entity, id)| (*id == target).then_some(entity))
        })
        .collect()
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Bevy system parameters own render-time animation state"
)]
pub(super) fn update_character_animation(
    imported: Option<Res<Imported3dAssets>>,
    time: Res<Time>,
    settings: Option<Res<ClientShellSettings>>,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    network_ids: Query<(Entity, &NetworkEntityId), With<Fighter>>,
    matches: Query<&MatchState, With<MatchRoot>>,
    mut runtimes: Query<(&mut V3CharacterRuntime, &mut Transform)>,
    owners: CharacterOwnerQuery,
    mut visuals: Query<&mut V3FighterVisual>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut bind_transforms: Query<&mut Transform, Without<V3CharacterRuntime>>,
) {
    let Some(imported) = imported else {
        cues.clear();
        return;
    };
    let clips = imported.clips;
    let delta = time.delta_secs();
    let phase = matches.iter().next().map(|state| state.phase);
    let reduced_motion = settings.is_some_and(|value| value.reduced_motion);
    let damaged = damaged_fighters(&mut cues, &network_ids);
    for (mut runtime, mut root_transform) in &mut runtimes {
        let Ok((health, defeated, team, loadout, ability)) = owners.get(runtime.owner) else {
            continue;
        };
        let Ok(mut visual) = visuals.get_mut(runtime.visual_root) else {
            continue;
        };
        visual.shoot_seconds = (visual.shoot_seconds - delta).max(0.0);
        let facts = FighterFacts {
            family: loadout.map_or(WeaponFamily::OneHanded, |loadout| {
                WeaponFamily::of(&loadout.primary_weapon.recipe.delivery)
            }),
            defeated: character_is_visually_defeated(health.0, defeated.is_some()),
            victorious: phase.is_some_and(|phase| team_won(phase, *team)),
            moving: visual.moving,
            hit: damaged.contains(&runtime.owner),
            shoot_seconds: visual.shoot_seconds,
            ability: ability.map(|state| state.phase),
        };
        let edges = runtime.advance_windows(&clips, facts, delta);
        let next = runtime.next_motion(facts);
        let respawned = runtime.current == CharacterMotion::Defeated && next != runtime.current;
        runtime.start_impulses(edges, respawned, delta);

        let restart = edges.fired && matches!(next, CharacterMotion::Attack(_));
        if (next != runtime.current || restart)
            && let Ok((mut player, mut transitions)) = players.get_mut(runtime.player)
        {
            if respawned {
                for (entity, bind_transform) in &runtime.bind_pose {
                    if let Ok(mut transform) = bind_transforms.get_mut(*entity) {
                        transform.clone_from(bind_transform);
                    }
                }
            }
            play_character_motion(
                &mut player,
                &mut transitions,
                clips.node(next),
                runtime.current,
                next,
            );
            runtime.current = next;
        }

        let run_phase = (runtime.current == CharacterMotion::Run)
            .then(|| players.get(runtime.player).ok())
            .flatten()
            .and_then(|(player, _)| player.animation(clips.run))
            .and_then(|animation| {
                (clips.run_seconds > 0.0)
                    .then(|| (animation.seek_time() / clips.run_seconds).rem_euclid(1.0))
            });
        root_transform.scale = runtime.squashed_scale(reduced_motion, run_phase);
    }
}

fn play_character_motion(
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    node: AnimationNodeIndex,
    previous: CharacterMotion,
    next: CharacterMotion,
) {
    let recovered_from_defeat =
        previous == CharacterMotion::Defeated && next != CharacterMotion::Defeated;
    if recovered_from_defeat {
        player.stop_all();
        *transitions = AnimationTransitions::new();
    }
    let duration = if recovered_from_defeat {
        Duration::ZERO
    } else {
        MOTION_BLEND
    };
    let animation = transitions.play(player, node, duration);
    if next.loops() {
        animation.repeat();
    }
}

#[cfg(test)]
mod tests;
