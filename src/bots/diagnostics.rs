use super::model::{BotRole, BotTactic};
use crate::{protocol::FighterInput, protocol::NetworkEntityId};
use bevy::prelude::*;
use std::collections::VecDeque;

const MAX_BOT_TRACES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BotDecisionTrace {
    pub tick: u64,
    pub network_id: NetworkEntityId,
    pub role: BotRole,
    pub tactic: BotTactic,
    pub input: FighterInput,
}

#[derive(Resource, Debug)]
pub(super) struct BotDiagnostics {
    pub decisions: u64,
    pub neutral_decisions: u64,
    pub invalid_outputs: u64,
    pub trace_drops: u64,
    pub total_controller_micros: u64,
    pub maximum_controller_micros: u64,
    pub navigation_searches_started: u64,
    pub navigation_searches_pending: u64,
    pub navigation_searches_completed: u64,
    pub navigation_searches_exhausted: u64,
    pub navigation_expansions: u64,
    trace_enabled: bool,
    pub traces: VecDeque<BotDecisionTrace>,
}

impl FromWorld for BotDiagnostics {
    fn from_world(_: &mut World) -> Self {
        Self {
            decisions: 0,
            neutral_decisions: 0,
            invalid_outputs: 0,
            trace_drops: 0,
            total_controller_micros: 0,
            maximum_controller_micros: 0,
            navigation_searches_started: 0,
            navigation_searches_pending: 0,
            navigation_searches_completed: 0,
            navigation_searches_exhausted: 0,
            navigation_expansions: 0,
            trace_enabled: std::env::var("BRAWLER_BOT_TRACE").as_deref() == Ok("1"),
            traces: VecDeque::with_capacity(MAX_BOT_TRACES),
        }
    }
}

impl BotDiagnostics {
    #[cfg(test)]
    pub(super) fn enable_trace_for_test(&mut self) {
        self.trace_enabled = true;
    }

    pub(super) fn record(&mut self, trace: BotDecisionTrace) {
        self.decisions = self.decisions.saturating_add(1);
        if trace.input == FighterInput::default() {
            self.neutral_decisions = self.neutral_decisions.saturating_add(1);
        }
        if !self.trace_enabled {
            return;
        }
        if self.traces.len() == MAX_BOT_TRACES {
            self.traces.pop_front();
            self.trace_drops = self.trace_drops.saturating_add(1);
        }
        self.traces.push_back(trace);
    }

    pub(super) fn record_invalid_output(&mut self) {
        self.invalid_outputs = self.invalid_outputs.saturating_add(1);
    }

    pub(super) fn record_controller_duration(&mut self, elapsed: std::time::Duration) {
        let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
        self.total_controller_micros = self.total_controller_micros.saturating_add(micros);
        self.maximum_controller_micros = self.maximum_controller_micros.max(micros);
    }

    pub(super) fn record_navigation(
        &mut self,
        value: super::policy::BotNavigationDecisionDiagnostics,
    ) {
        self.navigation_searches_started = self
            .navigation_searches_started
            .saturating_add(u64::from(value.search_started));
        match value.status {
            super::policy::BotNavigationSearchStatus::None => {}
            super::policy::BotNavigationSearchStatus::Pending => {
                self.navigation_searches_pending =
                    self.navigation_searches_pending.saturating_add(1);
            }
            super::policy::BotNavigationSearchStatus::Completed => {
                self.navigation_searches_completed =
                    self.navigation_searches_completed.saturating_add(1);
            }
            super::policy::BotNavigationSearchStatus::Exhausted => {
                self.navigation_searches_exhausted =
                    self.navigation_searches_exhausted.saturating_add(1);
            }
        }
        self.navigation_expansions = self
            .navigation_expansions
            .saturating_add(u64::try_from(value.expansions).unwrap_or(u64::MAX));
    }
}
