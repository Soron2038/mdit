use mdit::editor::tab_manager::active_index_after_close;

// ── active_index_after_close ───────────────────────────────────────────

#[test]
fn close_only_tab() {
    // 1 tab, close index 0 → 0 (caller handles "last tab" case)
    assert_eq!(active_index_after_close(1, 0, 0), 0);
}

#[test]
fn close_active_first_of_two() {
    // [A*, B] → close 0 → B becomes active at 0
    assert_eq!(active_index_after_close(2, 0, 0), 0);
}

#[test]
fn close_active_last_of_two() {
    // [A, B*] → close 1 → A becomes active at 0
    assert_eq!(active_index_after_close(2, 1, 1), 0);
}

#[test]
fn close_inactive_before_active() {
    // [A, B, C*] → close 0 → C shifts to index 1
    assert_eq!(active_index_after_close(3, 2, 0), 1);
}

#[test]
fn close_inactive_after_active() {
    // [A*, B, C] → close 2 → A stays at 0
    assert_eq!(active_index_after_close(3, 0, 2), 0);
}

#[test]
fn close_active_middle_of_three() {
    // [A, B*, C] → close 1 → new active is 0 (shift left)
    assert_eq!(active_index_after_close(3, 1, 1), 0);
}

#[test]
fn close_tab_before_active_in_four() {
    // [A, B, C*, D] → close 1 → C shifts to index 1
    assert_eq!(active_index_after_close(4, 2, 1), 1);
}

#[test]
fn close_last_tab_active_is_first() {
    // [A*, B, C] → close 2 → A stays at 0
    assert_eq!(active_index_after_close(3, 0, 2), 0);
}

#[test]
fn close_first_active_is_last() {
    // [A, B, C*] → close 0 → C shifts to index 1
    assert_eq!(active_index_after_close(3, 2, 0), 1);
}

#[test]
fn close_same_as_active_at_end() {
    // [A, B, C, D*] → close 3 → clamp to 2
    assert_eq!(active_index_after_close(4, 3, 3), 2);
}

#[test]
#[should_panic(expected = "cannot compute close index for empty tab list")]
fn close_from_empty_panics() {
    active_index_after_close(0, 0, 0);
}
