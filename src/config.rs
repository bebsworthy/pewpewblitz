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
    Takedown,
    TurfHold,
    VaultBreak,
    Zombie,
}

impl GameMode {
    pub const ALL: [Self; 4] = [
        Self::Takedown,
        Self::TurfHold,
        Self::VaultBreak,
        Self::Zombie,
    ];

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "takedown" | "default" => Some(Self::Takedown),
            "turf-hold" | "turf_hold" | "turfhold" => Some(Self::TurfHold),
            "vault-break" | "vault_break" | "vaultbreak" => Some(Self::VaultBreak),
            "zombie" => Some(Self::Zombie),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Takedown => "takedown",
            Self::TurfHold => "turf-hold",
            Self::VaultBreak => "vault-break",
            Self::Zombie => "zombie",
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
    pub match_spawn_protection_ticks: Option<u64>,
    pub match_completed_input_lock_ticks: Option<u64>,
    pub takedown_recent_hostile_damage_credit_ticks: Option<u64>,
    pub vault_break_critical_health_percent: Option<u8>,
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
            game_mode: GameMode::Takedown,
            match_rules_profile: MatchRulesProfile::Production,
            match_objective_target: None,
            match_duration_ticks: None,
            match_countdown_ticks: None,
            match_respawn_ticks: None,
            match_spawn_protection_ticks: None,
            match_completed_input_lock_ticks: None,
            takedown_recent_hostile_damage_credit_ticks: None,
            vault_break_critical_health_percent: None,
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
            self.match_spawn_protection_ticks.is_some(),
            self.match_completed_input_lock_ticks.is_some(),
            self.takedown_recent_hostile_damage_credit_ticks.is_some(),
            self.vault_break_critical_health_percent.is_some(),
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
            || self.match_spawn_protection_ticks == Some(0)
            || self.match_completed_input_lock_ticks == Some(0)
            || self.takedown_recent_hostile_damage_credit_ticks == Some(0)
        {
            return Err("resolved match rule values must be greater than zero".to_string());
        }
        if self
            .vault_break_critical_health_percent
            .is_some_and(|percent| !(1..=99).contains(&percent))
        {
            return Err(
                "resolved Vault Break critical-health percentage must be within 1..=99".to_string(),
            );
        }
        Ok(())
    }
}

/// Runtime configuration for one client process. Automation-only settings (smoke drivers,
/// scripted input, demos, scheduled captures, render reports) live in `brawler-client`'s
/// `ClientAutomationConfig`, which exists only with that package's `automation` feature.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ClientNetworkConfig {
    pub server_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub transport: NetworkTransport,
    pub network_protocol_id: u64,
    pub client_id: u64,
    /// Lobby account for clients without a persisted per-server account (headless automation and
    /// tests). `None` uses `client_id`. Setting it lets a fresh Netcode client present the account
    /// of an earlier one, as an app relaunched after being killed does.
    pub deterministic_account_id: Option<u128>,
    /// Editable logical address prefill for the interactive product shell.
    pub product_server_prefill: Option<String>,
    pub expected_protocol_version: u16,
    pub expected_build_version: String,
    pub connect_timeout: Duration,
    pub impairment_profile: NetworkImpairmentProfile,
    /// Run without a window, presentation, or product shell (automation and separate-App tests).
    pub headless: bool,
    /// Connect immediately instead of presenting the windowed product shell. Automation sets it
    /// for every non-interactive run, including the windowed combat and controller demos.
    pub auto_connect: bool,
    pub render_profile: RenderProfile,
    pub window_size: Option<(u16, u16)>,
}

/// Draws a nonzero client identity from operating-system entropy.
pub fn random_nonzero_client_id() -> Result<u64, String> {
    for _ in 0..4 {
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes)
            .map_err(|error| format!("OS entropy unavailable for client identity: {error}"))?;
        let id = u64::from_ne_bytes(bytes);
        if id != 0 {
            return Ok(id);
        }
    }
    Err("OS entropy repeatedly produced a zero client identity".to_string())
}

impl ClientNetworkConfig {
    /// The interactive product shell shared by the desktop player binary and the Android library:
    /// routed UDP, a random client identity, and an optional editable server prefill.
    pub fn product_shell(
        server_prefill: Option<String>,
        window_size: Option<(u16, u16)>,
    ) -> Result<Self, String> {
        let mut config = Self::new(random_nonzero_client_id()?);
        config.product_server_prefill = server_prefill;
        config.transport = NetworkTransport::RoutedUdp;
        config.window_size = window_size;
        config
            .validate()
            .map_err(|error| format!("invalid client configuration: {error}"))?;
        Ok(config)
    }

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
            deterministic_account_id: None,
            product_server_prefill: None,
            expected_protocol_version: crate::protocol::SUPPORTED_PROTOCOL_VERSION,
            expected_build_version: crate::VERSION.to_string(),
            connect_timeout: Duration::from_secs(5),
            impairment_profile: NetworkImpairmentProfile::from_env(),
            headless: false,
            auto_connect: false,
            render_profile: RenderProfile::from_env(),
            window_size: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.window_size.is_some_and(|(width, height)| {
            !(640..=3_840).contains(&width) || !(360..=2_160).contains(&height)
        }) {
            return Err("--window-size must be between 640x360 and 3840x2160".to_string());
        }
        if self.connect_timeout.is_zero() {
            return Err("client connect timeout must be greater than zero".to_string());
        }
        if self.expected_build_version.is_empty() {
            return Err("expected build version must not be empty".to_string());
        }
        Ok(())
    }

    /// Whether startup must create a Lightyear client entity immediately.
    #[must_use]
    pub const fn connects_on_startup(&self) -> bool {
        self.headless || self.auto_connect
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
    fn game_modes_parse_explicit_names_and_default_to_takedown() {
        assert_eq!(ServerNetworkConfig::default().game_mode, GameMode::Takedown);
        assert_eq!(GameMode::parse("takedown"), Some(GameMode::Takedown));
        assert_eq!(GameMode::parse("turf-hold"), Some(GameMode::TurfHold));
        assert_eq!(GameMode::parse("Turf_Hold"), Some(GameMode::TurfHold));
        assert_eq!(GameMode::parse("vault-break"), Some(GameMode::VaultBreak));
        assert_eq!(GameMode::parse("Vault_Break"), Some(GameMode::VaultBreak));
        assert_eq!(GameMode::parse("koth"), None);
        assert_eq!(GameMode::TurfHold.name(), "turf-hold");
        assert_eq!(GameMode::VaultBreak.name(), "vault-break");
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
    fn resolved_match_policy_is_complete_and_bounded() {
        let partial = ServerNetworkConfig {
            match_objective_target: Some(10),
            ..ServerNetworkConfig::default()
        };
        assert!(
            partial
                .validate()
                .is_err_and(|error| error.contains("complete set"))
        );

        let mut config = ServerNetworkConfig {
            match_objective_target: Some(10),
            match_duration_ticks: Some(10_800),
            match_countdown_ticks: Some(180),
            match_respawn_ticks: Some(180),
            match_spawn_protection_ticks: Some(90),
            match_completed_input_lock_ticks: Some(60),
            takedown_recent_hostile_damage_credit_ticks: Some(300),
            vault_break_critical_health_percent: Some(25),
            ..ServerNetworkConfig::default()
        };
        assert!(config.validate().is_ok());

        config.vault_break_critical_health_percent = Some(100);
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("1..=99"))
        );
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
    fn product_shell_and_startup_connection_are_mutually_exclusive() {
        let mut config = ClientNetworkConfig::new(1);
        assert!(config.presents_product_shell());
        assert!(!config.connects_on_startup());

        config.auto_connect = true;
        assert!(!config.presents_product_shell());
        assert!(config.connects_on_startup());

        config.auto_connect = false;
        config.headless = true;
        assert!(!config.presents_product_shell());
        assert!(config.connects_on_startup());
    }
}
