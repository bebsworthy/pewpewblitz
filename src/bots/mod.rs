//! Private server-hosted Practice bot composition.

#[cfg(feature = "server")]
mod behaviors;
#[cfg(feature = "server")]
mod controller;
#[cfg(feature = "server")]
mod diagnostics;
#[cfg(feature = "server")]
mod entropy;
#[cfg(feature = "server")]
mod model;
#[cfg(feature = "server")]
mod navigation;
#[cfg(feature = "server")]
mod policy;
mod profile;
#[cfg(feature = "server")]
mod team;

#[cfg(feature = "server")]
pub(crate) use controller::install_controller_systems;
#[cfg(feature = "server")]
pub(crate) use model::PracticeBotController;
pub(crate) use profile::BotCatalog;

#[cfg(all(test, feature = "server"))]
mod tests;
