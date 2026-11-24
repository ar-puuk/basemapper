//! What basemapper decided for the user, and what it could not do: notes
//! carried on a view and its rendering (docs/design/api-review.md §4.8).
//!
//! The level decides how R and Python surface a note: *decided* never
//! interrupts (shown on request), *adjusted* is a message once per kind per
//! session, *problem* is a warning every time.

/// How much a note matters to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// A value the user left open, chosen by basemapper.
    Decided,
    /// Something the user asked for was changed, or settled for less.
    Adjusted,
    /// The output is incomplete or likely not what was meant.
    Problem,
}

impl Level {
    /// `"decided"`, `"adjusted"` or `"problem"`.
    pub fn name(self) -> &'static str {
        match self {
            Level::Decided => "decided",
            Level::Adjusted => "adjusted",
            Level::Problem => "problem",
        }
    }
}

/// One note: its level, a stable kind (for code to match on), and a message
/// for people.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub level: Level,
    /// A stable identifier, e.g. `"effective_dpi"`, `"view_constrained"`.
    pub kind: &'static str,
    pub message: String,
}

impl Note {
    pub fn new(level: Level, kind: &'static str, message: impl Into<String>) -> Self {
        Note { level, kind, message: message.into() }
    }
}
