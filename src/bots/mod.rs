//! Private server-hosted Practice bot composition.

mod controller;
mod diagnostics;
mod entropy;
mod model;
mod navigation;
mod policy;
mod profile;
mod team;

pub(crate) use controller::install_controller_systems;
pub(crate) use model::PracticeBotController;

#[cfg(test)]
mod tests;
