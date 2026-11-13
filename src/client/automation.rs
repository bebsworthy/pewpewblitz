//! Automation-only client drivers, compiled only with the `automation` feature: headless
//! gameplay-intent synthesis for scripted smoke runs and the scheduled paired-capture plan used
//! by native visual verification. The player build never contains this module.
#![allow(clippy::wildcard_imports)]

use super::*;

const HEADLESS_FIRE_DURATION_TICKS: u32 = 6_000;

pub(super) fn install_scheduled_screenshots(
    app: &mut App,
    schedule: crate::config::ScreenshotSchedule,
) {
    std::fs::create_dir_all(&schedule.dir).expect("screenshot directory is creatable");
    app.insert_resource(ScheduledScreenshots {
        dir: schedule.dir,
        first_update: schedule.first_update,
        interval: schedule.interval,
        remaining: schedule.count,
        update_index: 0,
        captured: 0,
    })
    .add_systems(Update, capture_scheduled_screenshot);
}

/// In-process paired frame/state capture state for windowed visual verification.
#[derive(Resource, Debug)]
struct ScheduledScreenshots {
    dir: std::path::PathBuf,
    first_update: u32,
    interval: u32,
    remaining: u32,
    update_index: u32,
    captured: u32,
}

impl ScheduledScreenshots {
    fn take_due(&mut self) -> Option<(std::path::PathBuf, String)> {
        if self.remaining == 0 {
            return None;
        }
        let current = self.update_index;
        self.update_index = current.saturating_add(1);
        let due = self.first_update + self.interval * self.captured;
        if current < due {
            return None;
        }
        self.remaining -= 1;
        self.captured += 1;
        Some((self.dir.clone(), format!("brawler-{current:06}")))
    }
}

fn capture_scheduled_screenshot(world: &mut World) {
    let capture = {
        let mut plan = world.resource_mut::<ScheduledScreenshots>();
        let Some(capture) = plan.take_due() else {
            return;
        };
        capture
    };
    info!(directory = %capture.0.display(), stem = %capture.1, "capturing scheduled frame + client state");
    evidence_capture::request_scheduled_evidence_capture(world, &capture.0, &capture.1);
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn apply_headless_input(
    automation: Res<HeadlessAutomation>,
    mut pending: ResMut<PendingLocalActions>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
    controlled: Query<
        (
            &Position,
            &crate::combat::TeamId,
            Option<&crate::builds::AbilityState>,
            Option<&crate::builds::ResolvedMatchLoadout>,
        ),
        (With<Fighter>, With<Controlled>),
    >,
    fighters: Query<
        (
            &NetworkEntityId,
            &Position,
            &crate::combat::TeamId,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    sentries: Query<&crate::abilities::SentryIdentity, With<crate::abilities::Sentry>>,
    sentry_owners: Query<
        (
            &crate::combat::TeamId,
            &crate::builds::AbilityState,
            &crate::builds::ResolvedMatchLoadout,
        ),
        With<Fighter>,
    >,
) {
    if !statuses
        .iter()
        .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    {
        return;
    }
    if headless_automation_complete(&automation) {
        return;
    }
    let controlled_fighter = controlled.iter().next();
    let controlled_position = controlled_fighter.map(|(position, _, _, _)| position.0);
    let enemy_sentry_deployed = controlled_fighter
        .is_some_and(|(_, team, _, _)| sentries.iter().any(|identity| identity.team_id != *team));
    let enemy_sentry_imminent = controlled_fighter.is_some_and(|(_, team, _, _)| {
        sentry_owners.iter().any(|(owner_team, ability, loadout)| {
            *owner_team != *team
                && loadout.ultimate.kind == crate::builds::UltimateKind::Sentry
                && (u32::from(ability.charge)
                    >= u32::from(loadout.ultimate.charge_policy.maximum) * 4 / 5
                    || matches!(
                        ability.phase,
                        crate::builds::AbilityPhase::Ready
                            | crate::builds::AbilityPhase::Deployed { .. }
                    ))
        })
    });
    let target_position = fighters
        .iter()
        .find(|(network_id, _, _, _)| network_id.0 == 0)
        .map(|(_, target, _, _)| target.0)
        .or_else(|| {
            let (controlled_position, controlled_team, _, _) = controlled_fighter?;
            fighters
                .iter()
                .filter(|(_, _, team, defeated)| **team != *controlled_team && defeated.is_none())
                .min_by(|(_, left, _, _), (_, right, _, _)| {
                    left.0
                        .distance_squared(controlled_position.0)
                        .total_cmp(&right.0.distance_squared(controlled_position.0))
                })
                .map(|(_, target, _, _)| target.0)
        });
    let aim_delta = controlled_position
        .zip(target_position)
        .map(|(position, target)| target - position)
        .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON);
    let movement_delta = controlled_position
        .zip(target_position)
        .and_then(|(position, target)| headless_navigation_delta(position, target));
    let aim_axis = if automation.aim_at_dummy {
        aim_delta.map(Vec2::normalize)
    } else {
        automation.aim_axis
    };
    let (ability, loadout) =
        controlled_fighter.map_or((None, None), |(_, _, ability, loadout)| (ability, loadout));
    let (ultimate_ready, sentry_ready, sentry_deployed) =
        headless_ultimate_state(automation.ultimate, ability, loadout);
    let move_axis = if automation.aim_at_dummy && automation.move_axis != Vec2::ZERO {
        headless_combat_move_axis(
            automation.move_axis,
            aim_delta,
            movement_delta,
            sentry_ready || sentry_deployed,
        )
    } else {
        automation.move_axis
    };
    if move_axis != Vec2::ZERO
        || aim_axis.is_some()
        || automation.aim_at_dummy
        || automation.fire
        || automation.ultimate
    {
        if automation.elapsed_ticks == 0 {
            info!(
                move_axis = ?move_axis,
                aim_axis = ?aim_axis,
                aim_at_dummy = automation.aim_at_dummy,
                fire = automation.fire,
                ultimate = automation.ultimate,
                "headless movement automation enabled"
            );
        }
        pending.move_axis = move_axis;
        pending.aim_axis = aim_axis;
        pending.aim_distance = if automation.aim_at_dummy {
            aim_delta.map(Vec2::length)
        } else {
            None
        };
        let fire_held = automation.fire
            && automation.elapsed_ticks < HEADLESS_FIRE_DURATION_TICKS
            && !enemy_sentry_deployed
            && !enemy_sentry_imminent;
        let ultimate_held = ultimate_ready && automation.elapsed_ticks.is_multiple_of(12);
        pending.held_buttons = (u8::from(fire_held) * FighterInput::PRIMARY_FIRE)
            | (u8::from(ultimate_held) * FighterInput::ULTIMATE);
        pending.active_device = ActiveInputDevice::KeyboardMouse;
    }
}

fn headless_automation_complete(automation: &HeadlessAutomation) -> bool {
    automation
        .simulation_ticks
        .is_some_and(|limit| automation.elapsed_ticks >= limit)
}

fn headless_navigation_delta(position: Vec2, target: Vec2) -> Option<Vec2> {
    let direct = target - position;
    let delta = if direct.x.abs() > 300.0 {
        if position.y < 170.0 {
            Vec2::Y
        } else if position.y > 190.0 {
            Vec2::NEG_Y
        } else {
            Vec2::new(direct.x.signum(), 0.0)
        }
    } else {
        direct
    };
    (delta.is_finite() && delta.length_squared() > f32::EPSILON).then_some(delta)
}

fn headless_ultimate_state(
    enabled: bool,
    ability: Option<&crate::builds::AbilityState>,
    loadout: Option<&crate::builds::ResolvedMatchLoadout>,
) -> (bool, bool, bool) {
    let ready = enabled
        && ability
            .is_some_and(|ability| matches!(ability.phase, crate::builds::AbilityPhase::Ready));
    let sentry =
        loadout.is_some_and(|loadout| loadout.ultimate.kind == crate::builds::UltimateKind::Sentry);
    let deployed = enabled
        && sentry
        && ability.is_some_and(|ability| {
            matches!(ability.phase, crate::builds::AbilityPhase::Deployed { .. })
        });
    (ready, ready && sentry, deployed)
}

fn headless_combat_move_axis(
    fallback: Vec2,
    target_delta: Option<Vec2>,
    navigation_delta: Option<Vec2>,
    sentry_standoff: bool,
) -> Vec2 {
    navigation_delta.map_or(fallback, |delta| {
        let target_distance = target_delta.map_or(f32::INFINITY, Vec2::length);
        if sentry_standoff && target_distance < 320.0 {
            -delta.normalize()
        } else if sentry_standoff && target_distance <= 400.0 {
            Vec2::ZERO
        } else if target_distance > 130.0 {
            delta.normalize()
        } else if target_distance < 90.0 {
            -delta.normalize()
        } else {
            Vec2::ZERO
        }
    })
}

pub(super) fn advance_headless_automation(
    mut automation: ResMut<HeadlessAutomation>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
) {
    if automation.simulation_ticks.is_some()
        && statuses
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    {
        automation.elapsed_ticks = automation.elapsed_ticks.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_evidence_capture_uses_stable_due_stems_and_stops_at_count() {
        let mut plan = ScheduledScreenshots {
            dir: std::path::PathBuf::from("target/captures"),
            first_update: 2,
            interval: 3,
            remaining: 2,
            update_index: 0,
            captured: 0,
        };

        assert_eq!(plan.take_due(), None);
        assert_eq!(plan.take_due(), None);
        assert_eq!(
            plan.take_due(),
            Some((
                std::path::PathBuf::from("target/captures"),
                "brawler-000002".to_string(),
            ))
        );
        assert_eq!(plan.take_due(), None);
        assert_eq!(plan.take_due(), None);
        assert_eq!(
            plan.take_due(),
            Some((
                std::path::PathBuf::from("target/captures"),
                "brawler-000005".to_string(),
            ))
        );
        assert_eq!(plan.take_due(), None);
        assert_eq!((plan.remaining, plan.captured), (0, 2));
    }

    #[test]
    fn headless_cover_lane_movement_is_bounded() {
        assert_eq!(
            headless_navigation_delta(Vec2::new(-768.0, 0.0), Vec2::new(768.0, 0.0)),
            Some(Vec2::Y)
        );
        assert_eq!(
            headless_navigation_delta(Vec2::new(-500.0, 180.0), Vec2::new(500.0, 180.0)),
            Some(Vec2::X)
        );
        assert_eq!(
            headless_combat_move_axis(Vec2::X, Some(Vec2::new(350.0, 0.0)), Some(Vec2::X), true),
            Vec2::ZERO
        );
        assert_eq!(
            headless_combat_move_axis(Vec2::X, Some(Vec2::new(100.0, 0.0)), Some(Vec2::X), false),
            Vec2::ZERO
        );
    }
}
