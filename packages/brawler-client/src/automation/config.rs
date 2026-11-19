//! Automation-only client configuration: smoke exits, scripted input, demos, scheduled captures,
//! and native render reports. It is a separate resource beside the shared
//! [`ClientNetworkConfig`] and exists only with the `automation` feature, so the player build
//! carries none of these fields or their validation messages.

use bevy::prelude::{App, Resource, Vec2};
use brawler::config::{ClientNetworkConfig, HeadlessAutomation, NetworkTransport};
use core::time::Duration;

/// Settings only automation callers set (the automation CLI and separate-App tests).
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is one independent automation CLI switch, validated together below"
)]
pub struct ClientAutomationConfig {
    pub exit_after_roster: Option<usize>,
    /// In routed headless automation, wait for the completed match to tear down and a fresh
    /// lobby session to be accepted before exiting successfully.
    pub exit_after_lobby_return: bool,
    /// Headless M03 boundary: exit successfully after one authenticated lobby welcome.
    pub exit_after_lobby_welcome: bool,
    /// Headless M04 evidence: join one advertised pool, observe the fresh count, cancel, observe
    /// the resulting fresh count, and exit without requesting worker allocation.
    pub product_queue_smoke: bool,
    /// Headless M05 evidence: join an exact product pool and exit only after authoritative Active.
    pub product_match_smoke: bool,
    /// Headless V11 evidence: start Practice for one exact advertised game and exit only after
    /// the ordinary routed match reaches authoritative Active.
    pub product_practice_smoke: bool,
    /// Headless M06 evidence: complete a product match, return through Results, submit Queue Again,
    /// and exit only after the fresh queue Join is accepted.
    pub product_requeue_smoke: bool,
    /// Headless BRL-0162 evidence: during the Practice match, drop all traffic for a few seconds,
    /// move to a new local source port, and exit only after the same match session resumes.
    pub product_network_change_smoke: bool,
    /// Headless BRL-yyeg evidence: during the Practice match, drop all traffic for longer than
    /// the 10 s routed session bound, return to a fresh lobby, and exit only after the automatic
    /// rejoin reclaims the same fighter in the same running match.
    pub product_rejoin_smoke: bool,
    /// Headless BRL-jxfy evidence, for one player-versus-player client: drop all traffic beyond the routed
    /// session bound, write this cue file so the opponent leaves (and its team forfeits) while
    /// the seat is held, then come back, rejoin the completed match and exit after its forfeit
    /// result was captured and a fresh lobby was accepted.
    pub product_completed_rejoin_cue: Option<std::path::PathBuf>,
    /// Headless opponent for rejoin checks: send Leave Match once this file exists while the
    /// match is Active (BRL-jxfy). A person can create it by hand during a device check.
    pub leave_match_cue: Option<std::path::PathBuf>,
    pub product_match_players_per_team: u8,
    pub product_match_game_type: Option<brawler::lobby::GameTypeId>,
    pub headless_move: Option<(i8, i8)>,
    pub headless_aim: Option<(i8, i8)>,
    pub headless_aim_at_dummy: bool,
    pub headless_fire: bool,
    pub headless_ultimate: bool,
    pub headless_simulation_ticks: Option<u32>,
    /// Requested automation-brawler weapon. Saved-brawler preparation reads it here; the legacy
    /// combat evidence reads the copy in [`HeadlessAutomation`] made at composition.
    pub weapon_preset: Option<u16>,
    pub windowed_combat_demo: Option<WindowedCombatDemo>,
    pub windowed_controller_demo: Option<WindowedControllerDemo>,
    pub screenshot_schedule: Option<ScreenshotSchedule>,
    /// Opens one named player-shell screen through flow actions and saves it into the
    /// screenshot directory instead of following the frame schedule (BRL-pw9k).
    pub shell_capture: Option<crate::ShellCaptureRoute>,
    pub render_measurement: Option<RenderMeasurementConfig>,
    /// Renders the windowed presentation into an off-screen image with no OS window, so a
    /// scripted capture never appears on screen or takes keyboard focus (BRL-g7ny).
    pub offscreen: bool,
}

impl Default for ClientAutomationConfig {
    fn default() -> Self {
        Self {
            exit_after_roster: None,
            exit_after_lobby_return: false,
            exit_after_lobby_welcome: false,
            product_queue_smoke: false,
            product_match_smoke: false,
            product_practice_smoke: false,
            product_requeue_smoke: false,
            product_network_change_smoke: false,
            product_rejoin_smoke: false,
            product_completed_rejoin_cue: None,
            leave_match_cue: None,
            product_match_players_per_team: 2,
            product_match_game_type: None,
            headless_move: None,
            headless_aim: None,
            headless_aim_at_dummy: false,
            headless_fire: false,
            headless_ultimate: false,
            headless_simulation_ticks: None,
            weapon_preset: None,
            windowed_combat_demo: None,
            windowed_controller_demo: None,
            screenshot_schedule: None,
            shell_capture: None,
            render_measurement: None,
            offscreen: false,
        }
    }
}

/// Enables the reproducible, windowed aim-at-dummy/fire smoke scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowedCombatDemo;

/// Enables a reproducible windowed smoke scenario through the native gamepad input path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowedControllerDemo;

/// In-process paired frame/state capture plan for windowed visual verification. Frames are read
/// from the render surface, so no operating-system screen-recording permission is needed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenshotSchedule {
    pub dir: std::path::PathBuf,
    pub first_update: u32,
    pub interval: u32,
    pub count: u32,
}

/// Bounded, opt-in native render evidence. Normal clients leave this absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderMeasurementConfig {
    pub report_path: std::path::PathBuf,
    pub warmup: Duration,
    pub measurement: Duration,
    /// Exercises the persisted accessibility policy in non-interactive native evidence runs.
    pub reduced_effects: bool,
}

/// Installs the automation configuration (default when the caller set none) and the runtime
/// [`HeadlessAutomation`] derived from it. Runs while `ClientNetworkPlugin` builds, so every
/// composition path — the executable and the separate-App tests — gets one copy.
pub(crate) fn install_automation_state(app: &mut App) {
    app.init_resource::<ClientAutomationConfig>();
    let runtime = app
        .world()
        .resource::<ClientAutomationConfig>()
        .headless_automation();
    app.insert_resource(runtime);
}

impl ClientAutomationConfig {
    /// Whether this run synthesizes gameplay intent or measures a native match (so it readies
    /// automatically instead of waiting for player input).
    #[must_use]
    pub fn drives_match(&self) -> bool {
        self.headless_simulation_ticks.is_some()
            || self.windowed_combat_demo.is_some()
            || self.windowed_controller_demo.is_some()
            || self.render_measurement.is_some()
    }

    /// The runtime automation state this configuration starts with.
    #[must_use]
    pub fn headless_automation(&self) -> HeadlessAutomation {
        HeadlessAutomation {
            move_axis: self
                .headless_move
                .map_or(Vec2::ZERO, |(x, y)| Vec2::new(f32::from(x), f32::from(y))),
            aim_axis: self
                .headless_aim
                .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y))),
            aim_at_dummy: self.headless_aim_at_dummy,
            fire: self.headless_fire,
            ultimate: self.headless_ultimate,
            simulation_ticks: self.headless_simulation_ticks,
            elapsed_ticks: 0,
            weapon_preset: self.weapon_preset,
        }
    }

    /// Validates automation settings against the process configuration they run with.
    pub fn validate(&self, network: &ClientNetworkConfig) -> Result<(), String> {
        let headless = network.headless;
        let routed = network.transport == NetworkTransport::RoutedUdp;
        if self.exit_after_roster.is_some_and(|count| count == 0) {
            return Err("--exit-after-roster must be greater than zero".to_string());
        }
        if self.exit_after_lobby_return && !headless {
            return Err("--exit-after-lobby-return requires --headless".to_string());
        }
        if self.exit_after_lobby_welcome && !headless {
            return Err("--exit-after-lobby-welcome requires --headless".to_string());
        }
        if self.exit_after_lobby_welcome && !routed {
            return Err("--exit-after-lobby-welcome requires --transport routed-udp".to_string());
        }
        if self.product_queue_smoke && !headless {
            return Err("--product-queue-smoke requires --headless".to_string());
        }
        if self.product_queue_smoke && !routed {
            return Err("--product-queue-smoke requires --transport routed-udp".to_string());
        }
        if self.product_queue_smoke && self.exit_after_lobby_welcome {
            return Err(
                "--product-queue-smoke conflicts with --exit-after-lobby-welcome".to_string(),
            );
        }
        if self.product_match_smoke && !headless && self.render_measurement.is_none() {
            return Err(
                "--product-match-smoke requires --headless or a bounded render report".to_string(),
            );
        }
        if self.product_match_smoke && !routed {
            return Err("--product-match-smoke requires --transport routed-udp".to_string());
        }
        if self.product_match_smoke && !matches!(self.product_match_players_per_team, 1..=3) {
            return Err("product match smoke requires 1v1, 2v2, or 3v3".to_string());
        }
        self.validate_product_game_type()?;
        if self.product_match_smoke && self.product_queue_smoke {
            return Err("product queue and match smokes are mutually exclusive".to_string());
        }
        if self.product_requeue_smoke && !self.product_match_smoke {
            return Err("--product-requeue-smoke requires product match automation".to_string());
        }
        if self.exit_after_lobby_return && !routed {
            return Err("--exit-after-lobby-return requires --transport routed-udp".to_string());
        }
        if self
            .headless_simulation_ticks
            .is_some_and(|ticks| ticks == 0)
        {
            return Err("--simulation-ticks must be greater than zero".to_string());
        }
        if let Some(preset) = self.weapon_preset {
            validate_selectable_weapon_preset(preset)?;
        }
        self.validate_scripted_input(headless)?;
        if headless && self.windowed_controller_demo.is_some() {
            return Err("--controller-demo requires a windowed client".to_string());
        }
        if self.windowed_combat_demo.is_some() && self.windowed_controller_demo.is_some() {
            return Err("--combat-demo and --controller-demo cannot be combined".to_string());
        }
        if headless && self.offscreen {
            return Err("--offscreen requires a windowed client".to_string());
        }
        self.validate_screenshot_schedule(headless)?;
        self.validate_render_measurement(headless, network)?;
        self.validate_shell_capture(network)?;
        Ok(())
    }

    /// A shell capture drives the interactive product shell only, and needs a capture directory.
    fn validate_shell_capture(&self, network: &ClientNetworkConfig) -> Result<(), String> {
        if self.shell_capture.is_none() {
            return Ok(());
        }
        if !network.presents_product_shell() {
            return Err(
                "--shell-capture requires the interactive product shell (no --auto-connect, --headless, or demo)"
                    .to_string(),
            );
        }
        if self.screenshot_schedule.is_none() {
            return Err("--shell-capture requires --screenshot-dir".to_string());
        }
        if self.render_measurement.is_some() {
            return Err("--shell-capture conflicts with --render-report".to_string());
        }
        Ok(())
    }

    fn validate_scripted_input(&self, headless: bool) -> Result<(), String> {
        let automation_enabled = headless
            || self.windowed_combat_demo.is_some()
            || self.windowed_controller_demo.is_some()
            || self.render_measurement.is_some();
        if automation_enabled {
            return Ok(());
        }
        let requirement = "requires --headless, --combat-demo, or --render-report";
        let scripted = [
            ("--move-axis", self.headless_move.is_some()),
            ("--aim-axis", self.headless_aim.is_some()),
            ("--fire", self.headless_fire),
            ("--ultimate", self.headless_ultimate),
            ("--aim-dummy", self.headless_aim_at_dummy),
            (
                "--simulation-ticks",
                self.headless_simulation_ticks.is_some(),
            ),
        ];
        match scripted.iter().find(|(_, set)| *set) {
            Some((flag, _)) => Err(format!("{flag} {requirement}")),
            None => Ok(()),
        }
    }

    fn validate_product_game_type(&self) -> Result<(), String> {
        if self.product_match_game_type.is_some() && !self.product_match_smoke {
            return Err("--product-game-type requires match automation".to_string());
        }
        if self.product_practice_smoke && !self.product_match_smoke {
            return Err("--product-practice-smoke requires product match automation".to_string());
        }
        if self.product_network_change_smoke && !self.product_practice_smoke {
            return Err("--product-network-change-smoke requires Practice automation".to_string());
        }
        if self.product_rejoin_smoke
            && (!self.product_practice_smoke || self.product_network_change_smoke)
        {
            return Err(
                "--product-rejoin-smoke requires Practice automation without the network-change smoke"
                    .to_string(),
            );
        }
        let pvp_match =
            self.product_match_smoke && !self.product_practice_smoke && !self.product_requeue_smoke;
        if self.product_completed_rejoin_cue.is_some()
            && (!pvp_match || self.leave_match_cue.is_some())
        {
            return Err(
                "--product-completed-rejoin-smoke requires PvP match automation without --leave-match-cue"
                    .to_string(),
            );
        }
        if self.leave_match_cue.is_some() && !pvp_match {
            return Err("--leave-match-cue requires PvP match automation".to_string());
        }
        Ok(())
    }

    fn validate_screenshot_schedule(&self, headless: bool) -> Result<(), String> {
        let Some(schedule) = &self.screenshot_schedule else {
            return Ok(());
        };
        if headless {
            return Err("--screenshot-dir requires a windowed client".to_string());
        }
        if schedule.interval == 0 || schedule.count == 0 {
            return Err("screenshot interval and count must be greater than zero".to_string());
        }
        Ok(())
    }

    fn validate_render_measurement(
        &self,
        headless: bool,
        network: &ClientNetworkConfig,
    ) -> Result<(), String> {
        let Some(measurement) = &self.render_measurement else {
            return Ok(());
        };
        if headless {
            return Err("--render-report requires a windowed client".to_string());
        }
        if measurement.report_path.as_os_str().is_empty() {
            return Err("--render-report requires a non-empty path".to_string());
        }
        let maximum = Duration::from_mins(2);
        if measurement.warmup.is_zero()
            || measurement.measurement.is_zero()
            || measurement.warmup > maximum
            || measurement.measurement > maximum
        {
            return Err(
                "render warm-up and measurement must be between 1 and 120 seconds".to_string(),
            );
        }
        let interval = crate::planned_render_frame_interval(network.render_profile, self.offscreen);
        let required = minimum_render_measurement(interval);
        if measurement.measurement < required {
            let target = if self.offscreen {
                "off-screen"
            } else {
                "windowed"
            };
            return Err(format!(
                "--render-measure-seconds {} cannot reach the report's {RENDER_REPORT_MINIMUM_SAMPLES}-sample gate at the {} Hz {target} {} cadence; use at least {} seconds",
                measurement.measurement.as_secs(),
                frames_per_second(interval),
                network.render_profile.name(),
                required.as_secs(),
            ));
        }
        Ok(())
    }
}

/// The locked minimum frame-sample count of a passing native render report.
pub(crate) const RENDER_REPORT_MINIMUM_SAMPLES: u32 = 1_200;

/// Extra measurement time over `RENDER_REPORT_MINIMUM_SAMPLES` frames. A schedule runner or vsync
/// loop never runs faster than its nominal rate: off-screen native runs paced at 120 Hz measured
/// 110-114 Hz (sleep overshoot), so 10% headroom left under 20 samples to spare; 20% still passes
/// at an effective 100 Hz.
const RENDER_SAMPLE_HEADROOM_PERCENT: u32 = 20;

/// The shortest whole-second measurement window that collects the minimum sample count, plus
/// headroom, at `interval`: 12 s at 120 Hz, 24 s at 60 Hz, 48 s at 30 Hz.
fn minimum_render_measurement(interval: Duration) -> Duration {
    let nanos = interval.as_nanos()
        * u128::from(RENDER_REPORT_MINIMUM_SAMPLES)
        * u128::from(100 + RENDER_SAMPLE_HEADROOM_PERCENT)
        / 100;
    let seconds = nanos.div_ceil(1_000_000_000);
    Duration::from_secs(u64::try_from(seconds).unwrap_or(u64::MAX))
}

fn frames_per_second(interval: Duration) -> u128 {
    1_000_000_000 / interval.as_nanos().max(1)
}

/// Accepts only a weapon ID a saved brawler can select: the costed weapons in the embedded
/// `content/catalogs/builds.ron`, so the bound follows the catalog instead of a literal.
fn validate_selectable_weapon_preset(preset: u16) -> Result<(), String> {
    let catalog = brawler::builds::BuildCatalog::embedded()?;
    if catalog
        .weapon_costs
        .iter()
        .any(|cost| cost.weapon_id.0 == preset)
    {
        return Ok(());
    }
    let ids = catalog
        .weapon_costs
        .iter()
        .map(|cost| cost.weapon_id.0.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "--weapon-preset must be a weapon ID from the build catalog ({ids})"
    ))
}

/// Whether automation may send its automatic match commands: it drives the match and, when a
/// roster target is set, the replicated roster reached it.
#[must_use]
pub(crate) fn automatic_match_command_enabled(
    config: &ClientAutomationConfig,
    roster_count: usize,
) -> bool {
    config.drives_match()
        && config
            .exit_after_roster
            .is_none_or(|target| roster_count >= target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::config::RenderProfile;

    /// A windowed client at the default render profile, whatever `BRAWLER_RENDER_PROFILE` says.
    fn windowed() -> ClientNetworkConfig {
        let mut network = ClientNetworkConfig::new(1);
        network.render_profile = RenderProfile::Native;
        network
    }

    fn headless() -> ClientNetworkConfig {
        let mut network = ClientNetworkConfig::new(1);
        network.headless = true;
        network
    }

    #[test]
    fn defaults_validate_and_roster_target_must_be_positive() {
        let mut config = ClientAutomationConfig::default();
        assert!(config.validate(&windowed()).is_ok());
        config.exit_after_roster = Some(0);
        assert!(config.validate(&windowed()).is_err());
    }

    #[test]
    fn windowed_combat_demo_allows_native_automation_flags() {
        let config = ClientAutomationConfig {
            windowed_combat_demo: Some(WindowedCombatDemo),
            headless_aim_at_dummy: true,
            headless_fire: true,
            ..ClientAutomationConfig::default()
        };
        assert!(config.validate(&windowed()).is_ok());
    }

    #[test]
    fn scripted_input_requires_an_automation_mode() {
        let config = ClientAutomationConfig {
            headless_fire: true,
            ..ClientAutomationConfig::default()
        };
        assert!(
            config
                .validate(&windowed())
                .is_err_and(|error| error.starts_with("--fire requires"))
        );
        assert!(config.validate(&headless()).is_ok());
    }

    #[test]
    fn render_measurement_is_windowed_and_bounded() {
        let mut config = ClientAutomationConfig {
            render_measurement: Some(RenderMeasurementConfig {
                report_path: "render.txt".into(),
                warmup: Duration::from_secs(10),
                measurement: Duration::from_secs(30),
                reduced_effects: false,
            }),
            headless_move: Some((1, 0)),
            ..ClientAutomationConfig::default()
        };
        assert!(config.validate(&windowed()).is_ok());
        assert!(
            config
                .validate(&headless())
                .is_err_and(|error| error.contains("windowed"))
        );
        config.render_measurement.as_mut().unwrap().measurement = Duration::from_secs(121);
        assert!(
            config
                .validate(&windowed())
                .is_err_and(|error| error.contains("120"))
        );
    }

    fn render_report_lasting(seconds: u64, offscreen: bool) -> ClientAutomationConfig {
        ClientAutomationConfig {
            render_measurement: Some(RenderMeasurementConfig {
                report_path: "render.txt".into(),
                warmup: Duration::from_secs(4),
                measurement: Duration::from_secs(seconds),
                reduced_effects: false,
            }),
            offscreen,
            ..ClientAutomationConfig::default()
        }
    }

    fn profiled(profile: RenderProfile) -> ClientNetworkConfig {
        let mut network = windowed();
        network.render_profile = profile;
        network
    }

    /// BRL-vh61: the shortest accepted window reaches the report's sample gate, with headroom, at
    /// the cadence the run will use, and a shorter one names the duration it needs.
    #[test]
    fn render_measurement_must_reach_the_sample_gate_at_the_planned_cadence() {
        let cases = [
            (RenderProfile::Native, true, 12),
            (RenderProfile::SixtyFps, true, 24),
            (RenderProfile::ThirtyFps, true, 48),
            (RenderProfile::HighRefresh, true, 24),
            (RenderProfile::Native, false, 24),
            (RenderProfile::SixtyFps, false, 24),
            (RenderProfile::ThirtyFps, false, 48),
            (RenderProfile::HighRefresh, false, 24),
        ];
        for (profile, offscreen, required) in cases {
            let network = profiled(profile);
            let interval = crate::planned_render_frame_interval(profile, offscreen);
            let planned_samples = Duration::from_secs(required).as_nanos() / interval.as_nanos();
            assert!(
                planned_samples >= u128::from(RENDER_REPORT_MINIMUM_SAMPLES),
                "{profile:?} offscreen={offscreen}"
            );
            assert!(
                render_report_lasting(required, offscreen)
                    .validate(&network)
                    .is_ok(),
                "{profile:?} offscreen={offscreen}"
            );
            let error = render_report_lasting(required - 1, offscreen)
                .validate(&network)
                .expect_err("one second short is rejected");
            assert!(
                error.contains(&format!("use at least {required} seconds"))
                    && error.contains("1200-sample gate"),
                "{error}"
            );
        }
    }

    #[test]
    fn the_render_evidence_launcher_defaults_reach_the_sample_gate_off_screen() {
        let native = profiled(RenderProfile::Native);
        // scripts/v3-render-evidence.sh: 10 s + 30 s, and Heist Practice 4 s + 12 s.
        assert!(render_report_lasting(30, true).validate(&native).is_ok());
        assert!(render_report_lasting(12, true).validate(&native).is_ok());
        assert_eq!(
            minimum_render_measurement(crate::planned_render_frame_interval(
                RenderProfile::Native,
                true
            )),
            Duration::from_secs(12)
        );
    }

    #[test]
    fn controller_demo_is_windowed_and_mutually_exclusive_with_combat_demo() {
        let mut config = ClientAutomationConfig {
            windowed_controller_demo: Some(WindowedControllerDemo),
            ..ClientAutomationConfig::default()
        };
        assert!(config.validate(&windowed()).is_ok());

        config.windowed_combat_demo = Some(WindowedCombatDemo);
        assert!(
            config
                .validate(&windowed())
                .is_err_and(|error| error.contains("cannot be combined"))
        );

        config.windowed_combat_demo = None;
        assert!(
            config
                .validate(&headless())
                .is_err_and(|error| error.contains("windowed client"))
        );
    }

    #[test]
    fn explicit_product_game_type_is_scoped_to_match_automation() {
        let mut network = headless();
        let mut config = ClientAutomationConfig {
            product_match_game_type: Some(brawler::lobby::GameTypeId::new("hot-zone-2v2").unwrap()),
            ..ClientAutomationConfig::default()
        };
        assert!(config.validate(&network).is_err());
        config.product_match_smoke = true;
        network.transport = NetworkTransport::RoutedUdp;
        assert!(config.validate(&network).is_ok());

        config.product_practice_smoke = true;
        assert!(config.validate(&network).is_ok());
    }

    #[test]
    fn headless_weapon_preset_is_bounded_by_the_build_catalog() {
        let network = headless();
        let mut config = ClientAutomationConfig {
            weapon_preset: Some(5),
            ..ClientAutomationConfig::default()
        };
        assert!(config.validate(&network).is_ok());
        config.weapon_preset = Some(6);
        assert!(config.validate(&network).is_ok());
        // The bound follows the build catalog's costed weapons, including Splash.
        let catalog = brawler::builds::BuildCatalog::embedded().expect("embedded build catalog");
        for cost in &catalog.weapon_costs {
            config.weapon_preset = Some(cost.weapon_id.0);
            assert!(
                config.validate(&network).is_ok(),
                "weapon {}",
                cost.weapon_id.0
            );
        }
        let unknown = catalog
            .weapon_costs
            .iter()
            .map(|cost| cost.weapon_id.0)
            .max()
            .expect("costed weapons")
            + 1;
        config.weapon_preset = Some(unknown);
        assert!(config.validate(&network).is_err());
        config.weapon_preset = Some(0);
        assert!(config.validate(&network).is_err());
    }

    #[test]
    fn shell_capture_needs_the_interactive_shell_and_a_capture_directory() {
        let mut config = ClientAutomationConfig {
            shell_capture: Some(crate::ShellCaptureRoute::Dashboard),
            ..ClientAutomationConfig::default()
        };
        assert!(
            config
                .validate(&windowed())
                .is_err_and(|error| error.contains("--screenshot-dir"))
        );
        config.screenshot_schedule = Some(ScreenshotSchedule {
            dir: "target/shell-capture".into(),
            first_update: 30,
            interval: 60,
            count: 1,
        });
        assert!(config.validate(&windowed()).is_ok());
        let mut connecting = windowed();
        connecting.auto_connect = true;
        assert!(
            config
                .validate(&connecting)
                .is_err_and(|error| error.contains("interactive product shell"))
        );
        assert!(config.validate(&headless()).is_err());
    }

    #[test]
    fn automatic_match_ready_waits_for_the_requested_roster() {
        let config = ClientAutomationConfig {
            headless_simulation_ticks: Some(2_000),
            exit_after_roster: Some(4),
            ..ClientAutomationConfig::default()
        };
        assert!(!automatic_match_command_enabled(&config, 2));
        assert!(automatic_match_command_enabled(&config, 4));
    }

    #[test]
    fn windowed_combat_demo_automatically_readies_the_match() {
        let config = ClientAutomationConfig {
            windowed_combat_demo: Some(WindowedCombatDemo),
            ..ClientAutomationConfig::default()
        };
        assert!(automatic_match_command_enabled(&config, 2));
    }

    #[test]
    fn render_measurement_automatically_readies_without_injecting_combat_input() {
        let config = ClientAutomationConfig {
            render_measurement: Some(RenderMeasurementConfig {
                report_path: "target/report.txt".into(),
                warmup: Duration::from_secs(10),
                measurement: Duration::from_secs(30),
                reduced_effects: false,
            }),
            ..ClientAutomationConfig::default()
        };
        assert!(automatic_match_command_enabled(&config, 2));
        assert!(config.windowed_combat_demo.is_none());
        assert!(!config.headless_automation().fire);
    }

    #[test]
    fn runtime_state_copies_scripted_intent_and_weapon() {
        let config = ClientAutomationConfig {
            headless_move: Some((1, -1)),
            headless_fire: true,
            headless_simulation_ticks: Some(30),
            weapon_preset: Some(3),
            ..ClientAutomationConfig::default()
        };
        let runtime = config.headless_automation();
        assert_eq!(runtime.move_axis, Vec2::new(1.0, -1.0));
        assert!(runtime.fire);
        assert_eq!(runtime.simulation_ticks, Some(30));
        assert_eq!(runtime.weapon_preset, Some(3));
        assert_eq!(runtime.elapsed_ticks, 0);
    }
}
