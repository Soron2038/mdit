/// NSTextAlignmentCenter (1) — used by text fields that need centered text.
pub(crate) const NS_TEXT_ALIGNMENT_CENTER: usize = 1;
/// NSTextAlignmentRight (2) — used by text fields that need right-aligned text.
pub(crate) const NS_TEXT_ALIGNMENT_RIGHT: usize = 2;

pub mod sidebar;
pub mod appearance;
pub mod tab_bar;
pub mod path_bar;
pub mod find_bar;
pub mod welcome_overlay;
pub use find_bar::FindBar;
