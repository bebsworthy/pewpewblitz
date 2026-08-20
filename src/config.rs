//! Validated process configuration shared by the client and dedicated server.

use bevy::prelude::Resource;
use core::{net::SocketAddr, time::Duration};
use lightyear::prelude::{LinkConditionerConfig, RecvLinkConditioner};

/// Windowed presentation profile used by the visual smoke-test workflow.
///
/// The default keeps the platform's normal vsync behavior. The explicit profiles make it
/// possible to repeat the same scenario at the milestone's 30 Hz, 60 Hz, and high-refresh paths
/// without changing the fixed authoritative simulation tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderProfile {
    #[default]
    Native,
    ThirtyFps,
    SixtyFps,
    HighRefresh,
}

impl RenderProfile {
    #[must_use]
    pub fn from_env() -> Self {
        std::env::var("BRAWLER_RENDER_PROFILE")
            .ok()
            .and_then(|value| Self::parse(&value))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "native" | "default" => Some(Self::Native),
            "30" | "30hz" | "30fps" => Some(Self::ThirtyFps),
            "60" | "60hz" | "60fps" => Some(Self::SixtyFps),
            "high" | "high-refresh" | "high_refresh" | "uncapped" => Some(Self::HighRefresh),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::ThirtyFps => "30fps",
            Self::SixtyFps => "60fps",
            Self::HighRefresh => "high-refresh",
        }
    }
}

/// Statistical receive-side impairment profile used by process and Crossbeam evidence runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkImpairmentProfile {
    #[default]
    Local,
    Typical,
    Adverse,
}

impl NetworkImpairmentProfile {
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("BRAWLER_NETWORK_PROFILE").as_deref() {
            Ok("typical") => Self::Typical,
            Ok("adverse") => Self::Adverse,
            _ => Self::Local,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Typical => "typical",
            Self::Adverse => "adverse",
        }
    }

    #[must_use]
    pub fn receive_conditioner(self) -> Option<RecvLinkConditioner> {
        let config = match self {
            Self::Local => return None,
            Self::Typical => LinkConditionerConfig::default()
                .with_incoming_latency(Duration::from_millis(25))
                .with_incoming_jitter(Duration::from_millis(5)),
            Self::Adverse => LinkConditionerConfig::default()
                .with_incoming_latency(Duration::from_millis(50))
                .with_incoming_jitter(Duration::from_millis(10))
                .with_fixed_loss(0.02),
        };
        Some(RecvLinkConditioner::new(config))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkTransport {
    Udp,
    /// Routed public UDP through the M01 supervisor.  Direct UDP remains the default baseline.
    RoutedUdp,
    #[cfg(feature = "network-test")]
    Crossbeam,
}

/// Server-selected game mode. The dedicated server installs exactly one mode's rules and
/// compatible map; clients learn the mode from replicated map and match state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GameMode {
    #[default]
    Wipeout,
    HotZone,
}

impl GameMode {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "wipeout" | "default" => Some(Self::Wipeout),
            "hot-zone" | "hot_zone" | "hotzone" => Some(Self::HotZone),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Wipeout => "wipeout",
            Self::HotZone => "hot-zone",
        }
    }
}

/// Explicit server-owned match rules profile. Production never changes rules from ambient
/// verification environment variables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatchRulesProfile {
    #[default]
    Production,
    ProcessVerification,
}

impl MatchRulesProfile {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "production" | "default" => Some(Self::Production),
            "verification" | "process-verification" | "process_verification" => {
                Some(Self::ProcessVerification)
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn routing_id(self) -> u8 {
        match self {
            Self::Production => 1,
            Self::ProcessVerification => 2,
        }
    }
}

/// Runtime configuration for the dedicated server.
#[derive(bevy::prelude::Resource, Clone, Debug, PartialEq, Eq)]
pub struct ServerNetworkConfig {
    pub bind_addr: SocketAddr,
    pub transport: NetworkTransport,
    pub network_protocol_id: u64,
    pub max_clients: usize,
    pub handshake_timeout: Duration,
    pub client_timeout: Duration,
    pub impairment_profile: NetworkImpairmentProfile,
    pub game_mode: GameMode,
    pub match_rules_profile: MatchRulesProfile,
    pub match_objective_target: Option<u16>,
    pub match_duration_ticks: Option<u64>,
    pub match_countdown_ticks: Option<u64>,
    pub match_respawn_ticks: Option<u64>,
}

impl Default for ServerNetworkConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:5000"
                .parse()
                .expect("default server address is valid"),
            transport: NetworkTransport::Udp,
            network_protocol_id: crate::protocol::NETWORK_PROTOCOL_ID,
            max_clients: 8,
            handshake_timeout: Duration::from_secs(3),
            client_timeout: Duration::from_secs(3),
            impairment_profile: NetworkImpairmentProfile::from_env(),
            game_mode: GameMode::Wipeout,
            match_rules_profile: MatchRulesProfile::Production,
            match_objective_target: None,
            match_duration_ticks: None,
            match_countdown_ticks: None,
            match_respawn_ticks: None,
        }
    }
}

impl ServerNetworkConfig {
    /// Validate values that would otherwise make a process run indefinitely or wrap IDs.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_clients == 0 {
            return Err("--max-clients must be greater than zero".to_string());
        }
        if self.handshake_timeout.is_zero() {
            return Err("--handshake-timeout-ms must be greater than zero".to_string());
        }
        if self.client_timeout.is_zero() {
            return Err("client timeout must be greater than zero".to_string());
        }
        let match_overrides = [
            self.match_objective_target.is_some(),
            self.match_duration_ticks.is_some(),
            self.match_countdown_ticks.is_some(),
            self.match_respawn_ticks.is_some(),
        ];
        if match_overrides.iter().any(|present| *present)
            && !match_overrides.iter().all(|present| *present)
        {
            return Err("resolved match rules must be supplied as one complete set".to_string());
        }
        if self.match_objective_target == Some(0)
            || self.match_duration_ticks == Some(0)
            || self.match_countdown_ticks == Some(0)
            || self.match_respawn_ticks == Some(0)
        {
            return Err("resolved match rule values must be greater than zero".to_string());
        }
        Ok(())
    }
}

/// Runtime configuration for one client process.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ClientNetworkConfig {
    pub server_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub transport: NetworkTransport,
    pub network_protocol_id: u64,
    pub client_id: u64,
    /// Editable logical address prefill for the interactive product shell.
    pub product_server_prefill: Option<String>,
    pub expected_protocol_version: u16,
    pub expected_build_version: String,
    pub connect_timeout: Duration,
    pub impairment_profile: NetworkImpairmentProfile,
    pub headless: bool,
    /// Connect immediately instead of presenting the windowed product shell.
    pub auto_connect: bool,
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
    /// Headless M06 evidence: complete a product match, return through Results, submit Queue Again,
    /// and exit only after the fresh queue Join is accepted.
    pub product_requeue_smoke: bool,
    pub product_match_players_per_team: u8,
    pub headless_move: Option<(i8, i8)>,
    pub headless_aim: Option<(i8, i8)>,
    pub headless_aim_at_dummy: bool,
    pub headless_fire: bool,
    pub headless_ultimate: bool,
    pub headless_simulation_ticks: Option<u32>,
    pub build_preset: Option<u16>,
    pub windowed_combat_demo: Option<WindowedCombatDemo>,
    pub windowed_controller_demo: Option<WindowedControllerDemo>,
    pub render_profile: RenderProfile,
    pub window_size: Option<(u16, u16)>,
    pub screenshot_schedule: Option<ScreenshotSchedule>,
    pub render_measurement: Option<RenderMeasurementConfig>,
}

/// Enables the reproducible, windowed aim-at-dummy/fire smoke scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowedCombatDemo;

/// In-process screenshot capture plan for windowed visual verification. Frames are read
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
}

/// Enables a reproducible windowed smoke scenario through the native gamepad input path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowedControllerDemo;

impl ClientNetworkConfig {
    #[must_use]
    pub fn new(client_id: u64) -> Self {
        Self {
            server_addr: "127.0.0.1:5000"
                .parse()
                .expect("default server address is valid"),
            local_addr: "127.0.0.1:0"
                .parse()
                .expect("default client address is valid"),
            transport: NetworkTransport::Udp,
            network_protocol_id: crate::protocol::NETWORK_PROTOCOL_ID,
            client_id,
            product_server_prefill: None,
            expected_protocol_version: crate::protocol::SUPPORTED_PROTOCOL_VERSION,
            expected_build_version: crate::VERSION.to_string(),
            connect_timeout: Duration::from_secs(5),
            impairment_profile: NetworkImpairmentProfile::from_env(),
            headless: false,
            auto_connect: false,
            exit_after_roster: None,
            exit_after_lobby_return: false,
            exit_after_lobby_welcome: false,
            product_queue_smoke: false,
            product_match_smoke: false,
            product_requeue_smoke: false,
            product_match_players_per_team: 2,
            headless_move: None,
            headless_aim: None,
            headless_aim_at_dummy: false,
            headless_fire: false,
            headless_ultimate: false,
            headless_simulation_ticks: None,
            build_preset: None,
            windowed_combat_demo: None,
            windowed_controller_demo: None,
            render_profile: RenderProfile::from_env(),
            window_size: None,
            screenshot_schedule: None,
            render_measurement: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.exit_after_roster.is_some_and(|count| count == 0) {
            return Err("--exit-after-roster must be greater than zero".to_string());
        }
        if self.exit_after_lobby_return && !self.headless {
            return Err("--exit-after-lobby-return requires --headless".to_string());
        }
        if self.exit_after_lobby_welcome && !self.headless {
            return Err("--exit-after-lobby-welcome requires --headless".to_string());
        }
        if self.exit_after_lobby_welcome && self.transport != NetworkTransport::RoutedUdp {
            return Err("--exit-after-lobby-welcome requires --transport routed-udp".to_string());
        }
        if self.product_queue_smoke && !self.headless {
            return Err("--product-queue-smoke requires --headless".to_string());
        }
        if self.product_queue_smoke && self.transport != NetworkTransport::RoutedUdp {
            return Err("--product-queue-smoke requires --transport routed-udp".to_string());
        }
        if self.product_queue_smoke && self.exit_after_lobby_welcome {
            return Err(
                "--product-queue-smoke conflicts with --exit-after-lobby-welcome".to_string(),
            );
        }
        if self.product_match_smoke && !self.headless && self.render_measurement.is_none() {
            return Err(
                "--product-match-smoke requires --headless or a bounded render report".to_string(),
            );
        }
        if self.product_match_smoke && self.transport != NetworkTransport::RoutedUdp {
            return Err("--product-match-smoke requires --transport routed-udp".to_string());
        }
        if self.product_match_smoke && !matches!(self.product_match_players_per_team, 1..=3) {
            return Err("product match smoke requires 1v1, 2v2, or 3v3".to_string());
        }
        if self.product_match_smoke && self.product_queue_smoke {
            return Err("product queue and match smokes are mutually exclusive".to_string());
        }
        if self.product_requeue_smoke && !self.product_match_smoke {
            return Err("--product-requeue-smoke requires product match automation".to_string());
        }
        if self.exit_after_lobby_return && self.transport != NetworkTransport::RoutedUdp {
            return Err("--exit-after-lobby-return requires --transport routed-udp".to_string());
        }
        if self
            .headless_simulation_ticks
            .is_some_and(|ticks| ticks == 0)
        {
            return Err("--simulation-ticks must be greater than zero".to_string());
        }
        if self
            .build_preset
            .is_some_and(|preset| !(1..=5).contains(&preset))
        {
            return Err("--build-preset must be between 1 and 5 (5 selects custom)".to_string());
        }
        if self.window_size.is_some_and(|(width, height)| {
            !(640..=3_840).contains(&width) || !(360..=2_160).contains(&height)
        }) {
            return Err("--window-size must be between 640x360 and 3840x2160".to_string());
        }
        let automation_enabled = self.headless
            || self.windowed_combat_demo.is_some()
            || self.windowed_controller_demo.is_some()
            || self.render_measurement.is_some();
        let automation_requirement = "requires --headless, --combat-demo, or --render-report";
        if self.headless_move.is_some() && !automation_enabled {
            return Err(format!("--move-axis {automation_requirement}"));
        }
        if self.headless_aim.is_some() && !automation_enabled {
            return Err(format!("--aim-axis {automation_requirement}"));
        }
        if self.headless_fire && !automation_enabled {
            return Err(format!("--fire {automation_requirement}"));
        }
        if self.headless_ultimate && !automation_enabled {
            return Err(format!("--ultimate {automation_requirement}"));
        }
        if self.headless_aim_at_dummy && !automation_enabled {
            return Err(format!("--aim-dummy {automation_requirement}"));
        }
        if self.headless_simulation_ticks.is_some() && !automation_enabled {
            return Err(format!("--simulation-ticks {automation_requirement}"));
        }
        if self.connect_timeout.is_zero() {
            return Err("client connect timeout must be greater than zero".to_string());
        }
        if self.headless && self.windowed_controller_demo.is_some() {
            return Err("--controller-demo requires a windowed client".to_string());
        }
        if self.windowed_combat_demo.is_some() && self.windowed_controller_demo.is_some() {
            return Err("--combat-demo and --controller-demo cannot be combined".to_string());
        }
        if self.expected_build_version.is_empty() {
            return Err("expected build version must not be empty".to_string());
        }
        self.validate_screenshot_schedule()?;
        self.validate_render_measurement()?;
        Ok(())
    }

    fn validate_screenshot_schedule(&self) -> Result<(), String> {
        let Some(schedule) = &self.screenshot_schedule else {
            return Ok(());
        };
        if self.headless {
            return Err("--screenshot-dir requires a windowed client".to_string());
        }
        if schedule.interval == 0 || schedule.count == 0 {
            return Err("screenshot interval and count must be greater than zero".to_string());
        }
        Ok(())
    }

    fn validate_render_measurement(&self) -> Result<(), String> {
        let Some(measurement) = &self.render_measurement else {
            return Ok(());
        };
        if self.headless {
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
        Ok(())
    }

    /// Whether startup must create a Lightyear client entity immediately.
    #[must_use]
    pub const fn connects_on_startup(&self) -> bool {
        self.headless
            || self.auto_connect
            || self.windowed_combat_demo.is_some()
            || self.windowed_controller_demo.is_some()
    }

    /// Whether the windowed process should present the offline product shell.
    #[must_use]
    pub const fn presents_product_shell(&self) -> bool {
        !self.headless && !self.connects_on_startup()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impairment_profiles_have_expected_directional_conditioners() {
        assert_eq!(NetworkImpairmentProfile::Local.name(), "local");
        assert!(
            NetworkImpairmentProfile::Local
                .receive_conditioner()
                .is_none()
        );
        assert!(
            NetworkImpairmentProfile::Typical
                .receive_conditioner()
                .is_some()
        );
        assert!(
            NetworkImpairmentProfile::Adverse
                .receive_conditioner()
                .is_some()
        );
    }

    #[test]
    fn game_modes_parse_explicit_names_and_default_to_wipeout() {
        assert_eq!(ServerNetworkConfig::default().game_mode, GameMode::Wipeout);
        assert_eq!(GameMode::parse("wipeout"), Some(GameMode::Wipeout));
        assert_eq!(GameMode::parse("hot-zone"), Some(GameMode::HotZone));
        assert_eq!(GameMode::parse("Hot_Zone"), Some(GameMode::HotZone));
        assert_eq!(GameMode::parse("koth"), None);
        assert_eq!(GameMode::HotZone.name(), "hot-zone");
    }

    #[test]
    fn match_rules_profile_is_explicit_and_defaults_to_production() {
        assert_eq!(
            ServerNetworkConfig::default().match_rules_profile,
            MatchRulesProfile::Production
        );
        assert_eq!(
            MatchRulesProfile::parse("verification"),
            Some(MatchRulesProfile::ProcessVerification)
        );
        assert_eq!(MatchRulesProfile::parse("unexpected"), None);
    }

    #[test]
    fn windowed_combat_demo_allows_native_automation_flags() {
        let mut config = ClientNetworkConfig::new(1);
        config.windowed_combat_demo = Some(WindowedCombatDemo);
        config.headless_aim_at_dummy = true;
        config.headless_fire = true;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn render_profiles_parse_all_visual_smoke_modes() {
        assert_eq!(RenderProfile::parse("native"), Some(RenderProfile::Native));
        assert_eq!(
            RenderProfile::parse("30fps"),
            Some(RenderProfile::ThirtyFps)
        );
        assert_eq!(RenderProfile::parse("60Hz"), Some(RenderProfile::SixtyFps));
        assert_eq!(
            RenderProfile::parse("high-refresh"),
            Some(RenderProfile::HighRefresh)
        );
        assert_eq!(RenderProfile::parse("unknown"), None);
        assert_eq!(RenderProfile::HighRefresh.name(), "high-refresh");
    }

    #[test]
    fn render_measurement_is_windowed_and_bounded() {
        let mut config = ClientNetworkConfig::new(1);
        config.render_measurement = Some(RenderMeasurementConfig {
            report_path: "render.txt".into(),
            warmup: Duration::from_secs(10),
            measurement: Duration::from_secs(30),
        });
        config.headless_move = Some((1, 0));
        assert!(config.validate().is_ok());
        config.headless = true;
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("windowed"))
        );
        config.headless = false;
        config.render_measurement.as_mut().unwrap().measurement = Duration::from_secs(121);
        assert!(config.validate().is_err_and(|error| error.contains("120")));
    }

    #[test]
    fn controller_demo_is_windowed_and_mutually_exclusive_with_combat_demo() {
        let mut config = ClientNetworkConfig::new(1);
        config.windowed_controller_demo = Some(WindowedControllerDemo);
        assert!(config.validate().is_ok());

        config.windowed_combat_demo = Some(WindowedCombatDemo);
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("cannot be combined"))
        );

        config.windowed_combat_demo = None;
        config.headless = true;
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("windowed client"))
        );
    }

    #[test]
    fn product_shell_and_startup_connection_are_mutually_exclusive() {
        let mut config = ClientNetworkConfig::new(1);
        assert!(config.presents_product_shell());
        assert!(!config.connects_on_startup());

        config.auto_connect = true;
        assert!(!config.presents_product_shell());
        assert!(config.connects_on_startup());

        config.auto_connect = false;
        config.windowed_combat_demo = Some(WindowedCombatDemo);
        assert!(!config.presents_product_shell());
        assert!(config.connects_on_startup());

        config.windowed_combat_demo = None;
        config.windowed_controller_demo = Some(WindowedControllerDemo);
        assert!(!config.presents_product_shell());
        assert!(config.connects_on_startup());

        config.windowed_controller_demo = None;
        config.headless = true;
        assert!(!config.presents_product_shell());
        assert!(config.connects_on_startup());
    }
}
