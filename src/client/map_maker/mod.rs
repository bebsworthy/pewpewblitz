//! Player-owned Map Maker documents and editing state.

mod model;
mod persistence;

pub use model::{MapDraftDocument, MapDraftId, MapDraftLibrary, MapEditorSession};
pub(crate) use persistence::{MapDraftStorage, delete_map_draft, load_map_drafts, save_map_draft};
