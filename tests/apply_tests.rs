use mdit::editor::apply::collect_code_block_infos;
use mdit::markdown::parser::parse;

#[test]
fn code_block_infos_collected() {
    let text = "```rust\nlet x = 1;\n```\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].text, "let x = 1;");
    assert!(infos[0].start_utf16 < infos[0].end_utf16);
}

#[test]
fn two_code_blocks_both_collected() {
    let text = "```\nfoo\n```\n\nsome text\n\n```\nbar\n```\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert_eq!(infos.len(), 2);
    assert_eq!(infos[0].text, "foo");
    assert_eq!(infos[1].text, "bar");
}

#[test]
fn no_code_blocks_returns_empty() {
    let text = "Just a paragraph.\n";
    let spans = parse(text);
    let infos = collect_code_block_infos(&spans, text);
    assert!(infos.is_empty());
}
