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
mod registry;
#[cfg(feature = "server")]
mod team;

#[cfg(feature = "server")]
pub(crate) use model::PracticeBotController;
#[cfg(test)]
pub(crate) use profile::BotCatalogResource;
pub(crate) use profile::BotContentPlugin;

#[cfg(feature = "server")]
pub(crate) fn install_controller_systems(app: &mut bevy::prelude::App) {
    app.add_plugins((
        registry::BotBehaviorRegistryPlugin,
        behaviors::BuiltInBotBehaviorsPlugin,
    ));
    controller::install_controller_systems(app);
}

#[cfg(all(test, feature = "server"))]
mod tests;
