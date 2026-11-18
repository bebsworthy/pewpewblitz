//! Text-field input shared by hardware keyboards and on-screen keyboards (BRL-06pz).
//!
//! Every shell text editor reads [`TypedText`]: hardware key presses and Bevy [`Ime::Commit`]
//! messages become the same [`TextEdit`]s, so committed input-method text goes through each
//! editor's existing validation and length bounds.
//!
//! While a field is being edited the flow publishes it as [`TextEntryFocus`]. A platform shell
//! without a hardware keyboard (Android) shows its on-screen keyboard for it, mirrors the field's
//! text into the input method, and replays the input method's changes with
//! [`TextEntry::edit_toward`]: deletions as Backspace key messages, inserted text as
//! [`Ime::Commit`], and the keyboard's action key as Enter. Desktop never enables IME on the
//! window, so desktop editing stays on key presses alone.

use bevy::{
    ecs::system::SystemParam,
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
};
use unicode_segmentation::UnicodeSegmentation as _;

/// The kind of field being edited; it picks the on-screen keyboard layout and how many Backspace
/// presses remove a span of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextEntryField {
    /// Server Select address (ASCII; Backspace removes one byte).
    ServerAddress,
    /// Server Select display name (Backspace removes one grapheme).
    PlayerName,
    /// Brawler creation or rename (Backspace removes one grapheme).
    BrawlerName,
    /// Map Maker new or renamed map (Backspace removes one character).
    MapName,
}

/// The field being edited, its text, and its caret (a byte offset into the text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEntry {
    field: TextEntryField,
    text: String,
    caret: usize,
}

/// One replayable change from an input method's text to the edited field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextEntryEdit {
    /// Backspace presses at the caret, in the field's own deletion units.
    pub backspaces: usize,
    /// Text inserted at the caret after the deletions (delivered as [`Ime::Commit`]).
    pub insert: String,
    /// The input method inserted a line break: confirm the field (Enter) after inserting.
    pub submit: bool,
}

impl TextEntry {
    pub(crate) fn new(field: TextEntryField, text: &str, caret: usize) -> Self {
        let caret = caret.min(text.len());
        let caret = if text.is_char_boundary(caret) {
            caret
        } else {
            text.len()
        };
        Self {
            field,
            text: text.to_string(),
            caret,
        }
    }

    #[must_use]
    pub fn field(&self) -> TextEntryField {
        self.field
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret in UTF-16 code units, the unit Android input methods use.
    #[must_use]
    pub fn caret_utf16(&self) -> usize {
        self.text[..self.caret].encode_utf16().count()
    }

    /// The Backspace presses and insertion that turn this field's text into `target`, an input
    /// method's edit of it at the caret. `None` when `target` changed text after the caret, which
    /// the field cannot replay; the caller then restores the input method to the field's text.
    #[must_use]
    pub fn edit_toward(&self, target: &str) -> Option<TextEntryEdit> {
        let (before, after) = self.text.split_at(self.caret);
        let target_before = target.strip_suffix(after)?;
        let prefix = before
            .char_indices()
            .zip(target_before.chars())
            .find(|((_, left), right)| left != right)
            .map_or_else(
                || before.len().min(target_before.len()),
                |((index, _), _)| index,
            );
        let removed_start = match self.field {
            TextEntryField::PlayerName | TextEntryField::BrawlerName => before
                .grapheme_indices(true)
                .map(|(index, _)| index)
                .chain(std::iter::once(before.len()))
                .take_while(|index| *index <= prefix)
                .last()
                .unwrap_or(0),
            TextEntryField::ServerAddress | TextEntryField::MapName => prefix,
        };
        let removed = &before[removed_start..];
        let backspaces = match self.field {
            TextEntryField::PlayerName | TextEntryField::BrawlerName => {
                removed.graphemes(true).count()
            }
            TextEntryField::ServerAddress | TextEntryField::MapName => removed.chars().count(),
        };
        let inserted = &target_before[removed_start..];
        let (insert, submit) = match inserted.find(['\n', '\r']) {
            Some(line_break) => (&inserted[..line_break], true),
            None => (inserted, false),
        };
        Some(TextEntryEdit {
            backspaces,
            insert: insert.to_string(),
            submit,
        })
    }
}

/// The field the shell is editing this frame, published in `Last`; `None` when no field is.
#[derive(Resource, Default, Debug)]
pub struct TextEntryFocus {
    active: Option<TextEntry>,
}

impl TextEntryFocus {
    #[must_use]
    pub fn active(&self) -> Option<&TextEntry> {
        self.active.as_ref()
    }

    pub(crate) fn set(&mut self, active: Option<TextEntry>) {
        self.active = active;
    }
}

/// One text-field edit from a key press or an input-method commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TextEdit {
    Backspace,
    Delete,
    Insert(String),
}

/// Hardware key presses, then input-method commits, as text-field edits.
#[derive(SystemParam)]
pub(crate) struct TypedText<'w, 's> {
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    ime: MessageReader<'w, 's, Ime>,
}

impl TypedText<'_, '_> {
    /// This frame's edits. Key presses come first: a platform shell replays an input method's
    /// deletions as Backspace keys and its insertion as one commit.
    pub(crate) fn edits(&mut self) -> Vec<TextEdit> {
        let mut edits = self
            .keyboard
            .read()
            .filter(|event| event.state == ButtonState::Pressed)
            .filter_map(|event| match event.key_code {
                KeyCode::Backspace => Some(TextEdit::Backspace),
                KeyCode::Delete => Some(TextEdit::Delete),
                _ => event
                    .text
                    .as_deref()
                    .map(|text| TextEdit::Insert(text.to_string())),
            })
            .collect::<Vec<_>>();
        edits.extend(self.ime.read().filter_map(|event| match event {
            Ime::Commit { value, .. } if !value.is_empty() => Some(TextEdit::Insert(value.clone())),
            _ => None,
        }));
        edits
    }
}

/// Registers the messages and focus resource every text editor uses.
pub(crate) fn add_text_entry(app: &mut App) {
    app.add_message::<KeyboardInput>()
        .add_message::<Ime>()
        .init_resource::<TextEntryFocus>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(field: TextEntryField, text: &str, caret: usize, target: &str) -> TextEntryEdit {
        TextEntry::new(field, text, caret)
            .edit_toward(target)
            .expect("replayable edit")
    }

    #[test]
    fn appends_deletes_and_replaces_at_the_caret() {
        let address = TextEntryField::ServerAddress;
        assert_eq!(
            edit(address, "10.0", 4, "10.0.0.1"),
            TextEntryEdit {
                backspaces: 0,
                insert: ".0.1".into(),
                submit: false
            }
        );
        assert_eq!(edit(address, "10.0", 4, "10.").backspaces, 1);
        // An autocorrect replacing the word before the caret.
        let replaced = edit(TextEntryField::MapName, "Teh Map", 3, "The Map");
        assert_eq!((replaced.backspaces, replaced.insert.as_str()), (2, "he"));
        // Text after the caret must survive, or the edit is not replayable.
        assert!(TextEntry::new(address, "ab", 1,).edit_toward("a").is_none());
    }

    #[test]
    fn name_deletions_count_graphemes_and_line_breaks_submit() {
        let family = "A👨‍👩‍👧";
        let deleted = edit(TextEntryField::PlayerName, family, family.len(), "A");
        assert_eq!((deleted.backspaces, deleted.insert.as_str()), (1, ""));
        // Removing one code point of the cluster deletes the cluster and re-inserts the rest.
        let partial = "A👨‍👩‍";
        let trimmed = edit(TextEntryField::BrawlerName, family, family.len(), partial);
        assert_eq!((trimmed.backspaces, trimmed.insert.as_str()), (1, "👨‍👩‍"));
        let submitted = edit(TextEntryField::PlayerName, "Ace", 3, "Aces\n");
        assert_eq!(
            submitted,
            TextEntryEdit {
                backspaces: 0,
                insert: "s".into(),
                submit: true
            }
        );
        assert_eq!(TextEntry::new(TextEntryField::PlayerName, "é", 1,).caret, 2);
        assert_eq!(
            TextEntry::new(TextEntryField::PlayerName, "😀a", 5).caret_utf16(),
            3
        );
    }
}
