//! Interface preferences persisted through the store's key/value table.
//!
//! Only presentation choices live here. Nothing in this module affects how a run is
//! executed, and the values are written to `~/.ymp2` metadata like every other ymp record.

use anyhow::Result;
use serde_json::{json, Value};
use ymp_storage::Store;

/// Key in the store's key/value table.
pub const KEY: &str = "ui.prefs";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// Identifier of the selected palette.
    pub theme: String,
    /// Show every attributed agent message in full instead of collapsed activity.
    pub details: bool,
    /// Show the context sidebar when the terminal is wide enough for it.
    pub sidebar: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: crate::theme::DEFAULT_THEME.to_owned(),
            details: false,
            sidebar: true,
        }
    }
}

impl Prefs {
    /// Read stored preferences. A missing, unreadable, or partial record yields defaults,
    /// because a presentation preference must never prevent the interface from starting.
    pub fn load(store: &Store) -> Self {
        let mut prefs = Prefs::default();
        let Ok(Some(value)) = store.value(KEY) else {
            return prefs;
        };
        if let Some(id) = value.get("theme").and_then(Value::as_str) {
            // An unknown identifier falls back to the default palette rather than failing.
            prefs.theme = crate::theme::theme(id).id.to_owned();
        }
        if let Some(details) = value.get("details").and_then(Value::as_bool) {
            prefs.details = details;
        }
        if let Some(sidebar) = value.get("sidebar").and_then(Value::as_bool) {
            prefs.sidebar = sidebar;
        }
        prefs
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        store.put_value(
            KEY,
            &json!({
                "theme": self.theme,
                "details": self.details,
                "sidebar": self.sidebar,
            }),
        )
    }
}
