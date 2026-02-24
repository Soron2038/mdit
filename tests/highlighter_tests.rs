use mdit::markdown::highlighter::highlight;

#[test]
fn highlights_rust_code() {
    let result = highlight("fn main() {}", "rust");
    assert!(!result.spans.is_empty(), "should produce highlight spans for Rust");
}

#[test]
fn unknown_language_falls_back_gracefully() {
    let result = highlight("some code", "foobar_no_such_lang");
    // Should still produce at least one span (plain text fallback)
    assert_eq!(result.spans.len(), 1, "unknown language: single unstyled span");
}

#[test]
fn highlights_python_code() {
    let result = highlight("def foo():\n    pass\n", "python");
    assert!(
        result.spans.len() > 1,
        "Python code should produce multiple highlight spans"
    );
}

#[test]
fn empty_code_returns_no_spans() {
    let result = highlight("", "rust");
    assert!(result.spans.is_empty(), "empty code should yield no spans");
}
