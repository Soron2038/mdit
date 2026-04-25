//! View mode for the editor: Viewer (read-only rendered) or Editor (raw with syntax highlighting).

/// Controls whether a document tab is in read-only rendered view or editable raw mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Read-only rendered Markdown with full visual styling and custom drawing.
    Viewer,
    /// Editable raw Markdown with monospace font and syntax highlighting.
    Editor,
}
