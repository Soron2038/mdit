//! mdit-core — plattformneutrale Logik für mdit.
//!
//! Diese Crate enthält Markdown-Parsing, Code-Highlighting, das
//! Render-Run-Modell, das Theme/Color-Scheme, Preferences-Persistenz und
//! Find-Algorithmen. Sie kennt kein UI-Toolkit. Konsumenten sind die
//! UI-Crates (vorerst nur die kommende `mdit-ui` mit GPUI).

pub mod document;
pub mod find;
pub mod markdown;
pub mod preferences;
pub mod render;
pub mod theme;
