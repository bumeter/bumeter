//! Language for backend-generated strings (notifications, tray, statusline). The UI translates on its own in `i18n.ts`.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Vi,
    En,
}

impl Lang {
    pub fn pick<'a>(self, vi: &'a str, en: &'a str) -> &'a str {
        match self {
            Lang::Vi => vi,
            Lang::En => en,
        }
    }
}
