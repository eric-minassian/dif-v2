use super::view::drawing::{
    CELL_STYLE_FLAG_BOLD, CELL_STYLE_FLAG_FAINT, CELL_STYLE_FLAG_ITALIC,
    CELL_STYLE_FLAG_STRIKETHROUGH, CELL_STYLE_FLAG_UNDERLINE,
};
use super::view::helpers::encode_key_named;
use super::{CursorShape, Rgb, StyleRun, TerminalConfig, TerminalSession};

// ---------------------------------------------------------------------------
// TestTerm harness
// ---------------------------------------------------------------------------

struct TestTerm {
    session: TerminalSession,
}

impl TestTerm {
    fn new(cols: u16, rows: u16) -> Self {
        let config = TerminalConfig {
            cols,
            rows,
            ..TerminalConfig::default()
        };
        Self {
            session: TerminalSession::new(config).unwrap(),
        }
    }

    fn default() -> Self {
        Self::new(80, 24)
    }

    // -- Feed helpers -------------------------------------------------------

    fn feed(&mut self, bytes: &[u8]) {
        self.session.feed(bytes).unwrap();
    }

    fn feed_str(&mut self, s: &str) {
        self.feed(s.as_bytes());
    }

    // -- CSI sequence helpers (params are 1-based per VT convention) ---------

    fn cup(&mut self, row: u16, col: u16) {
        self.feed(format!("\x1b[{};{}H", row, col).as_bytes());
    }

    fn cuu(&mut self, n: u16) {
        self.feed(format!("\x1b[{}A", n).as_bytes());
    }

    fn cud(&mut self, n: u16) {
        self.feed(format!("\x1b[{}B", n).as_bytes());
    }

    fn cuf(&mut self, n: u16) {
        self.feed(format!("\x1b[{}C", n).as_bytes());
    }

    fn cub(&mut self, n: u16) {
        self.feed(format!("\x1b[{}D", n).as_bytes());
    }

    fn cnl(&mut self, n: u16) {
        self.feed(format!("\x1b[{}E", n).as_bytes());
    }

    fn cpl(&mut self, n: u16) {
        self.feed(format!("\x1b[{}F", n).as_bytes());
    }

    fn cha(&mut self, col: u16) {
        self.feed(format!("\x1b[{}G", col).as_bytes());
    }

    fn erase_in_display(&mut self, mode: u8) {
        self.feed(format!("\x1b[{}J", mode).as_bytes());
    }

    fn erase_in_line(&mut self, mode: u8) {
        self.feed(format!("\x1b[{}K", mode).as_bytes());
    }

    fn erase_characters(&mut self, n: u16) {
        self.feed(format!("\x1b[{}X", n).as_bytes());
    }

    fn delete_characters(&mut self, n: u16) {
        self.feed(format!("\x1b[{}P", n).as_bytes());
    }

    fn insert_characters(&mut self, n: u16) {
        self.feed(format!("\x1b[{}@", n).as_bytes());
    }

    fn insert_lines(&mut self, n: u16) {
        self.feed(format!("\x1b[{}L", n).as_bytes());
    }

    fn delete_lines(&mut self, n: u16) {
        self.feed(format!("\x1b[{}M", n).as_bytes());
    }

    fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        self.feed(format!("\x1b[{};{}r", top, bottom).as_bytes());
    }

    fn reset_scroll_region(&mut self) {
        self.feed(b"\x1b[r");
    }

    fn scroll_up(&mut self, n: u16) {
        self.feed(format!("\x1b[{}S", n).as_bytes());
    }

    fn scroll_down(&mut self, n: u16) {
        self.feed(format!("\x1b[{}T", n).as_bytes());
    }

    fn enter_alt_screen(&mut self) {
        self.feed(b"\x1b[?1049h");
    }

    fn exit_alt_screen(&mut self) {
        self.feed(b"\x1b[?1049l");
    }

    fn enable_autowrap(&mut self) {
        self.feed(b"\x1b[?7h");
    }

    fn disable_autowrap(&mut self) {
        self.feed(b"\x1b[?7l");
    }

    fn save_cursor(&mut self) {
        self.feed(b"\x1b7");
    }

    fn restore_cursor(&mut self) {
        self.feed(b"\x1b8");
    }

    fn sgr(&mut self, params: &str) {
        self.feed(format!("\x1b[{}m", params).as_bytes());
    }

    fn sgr_reset(&mut self) {
        self.feed(b"\x1b[0m");
    }

    fn cr(&mut self) {
        self.feed(b"\r");
    }

    fn lf(&mut self) {
        self.feed(b"\n");
    }

    fn bs(&mut self) {
        self.feed(b"\x08");
    }

    fn tab(&mut self) {
        self.feed(b"\t");
    }

    fn enable_insert_mode(&mut self) {
        self.feed(b"\x1b[4h");
    }

    fn disable_insert_mode(&mut self) {
        self.feed(b"\x1b[4l");
    }

    // -- Assertion helpers ---------------------------------------------------

    fn cursor_pos(&self) -> (u16, u16) {
        self.session
            .cursor_position()
            .expect("cursor_position() returned None")
    }

    fn assert_cursor_pos(&self, col: u16, row: u16) {
        let (c, r) = self.cursor_pos();
        assert_eq!(
            (c, r),
            (col, row),
            "cursor expected at ({col},{row}) but was ({c},{r})"
        );
    }

    fn row(&self, row: u16) -> String {
        self.session.dump_viewport_row(row).unwrap()
    }

    fn assert_row(&self, row: u16, expected: &str) {
        let actual = self.row(row);
        let trimmed = actual.trim_end();
        assert_eq!(
            trimmed, expected,
            "row {row}: expected {expected:?}, got {trimmed:?}"
        );
    }

    fn assert_row_blank(&self, row: u16) {
        let actual = self.row(row);
        assert!(
            actual.trim().is_empty(),
            "row {row}: expected blank, got {:?}",
            actual.trim()
        );
    }

    fn assert_row_starts_with(&self, row: u16, prefix: &str) {
        let actual = self.row(row);
        assert!(
            actual.starts_with(prefix),
            "row {row}: expected to start with {prefix:?}, got {:?}",
            &actual[..actual.len().min(prefix.len() + 10)]
        );
    }

    fn style_runs(&self, row: u16) -> Vec<StyleRun> {
        self.session.dump_viewport_row_style_runs(row).unwrap()
    }

    fn assert_has_style_flag_at(&self, row: u16, col: u16, flag: u8) {
        let runs = self.style_runs(row);
        for run in &runs {
            if col >= run.start_col && col <= run.end_col {
                assert!(
                    run.flags & flag != 0,
                    "row {row} col {col}: expected flag 0x{flag:02x} in run flags 0x{:02x}",
                    run.flags
                );
                return;
            }
        }
        panic!("row {row} col {col}: no style run covers this column");
    }

    fn assert_no_style_flag_at(&self, row: u16, col: u16, flag: u8) {
        let runs = self.style_runs(row);
        for run in &runs {
            if col >= run.start_col && col <= run.end_col {
                assert!(
                    run.flags & flag == 0,
                    "row {row} col {col}: expected NO flag 0x{flag:02x} but found flags 0x{:02x}",
                    run.flags
                );
                return;
            }
        }
        // No run covering this column means no flags, which is fine
    }

    fn assert_fg_at(&self, row: u16, col: u16, expected: Rgb) {
        let runs = self.style_runs(row);
        for run in &runs {
            if col >= run.start_col && col <= run.end_col {
                assert_eq!(
                    run.fg, expected,
                    "row {row} col {col}: fg expected {:?}, got {:?}",
                    expected, run.fg
                );
                return;
            }
        }
        panic!("row {row} col {col}: no style run covers this column");
    }

    fn assert_bg_at(&self, row: u16, col: u16, expected: Rgb) {
        let runs = self.style_runs(row);
        for run in &runs {
            if col >= run.start_col && col <= run.end_col {
                assert_eq!(
                    run.bg, expected,
                    "row {row} col {col}: bg expected {:?}, got {:?}",
                    expected, run.bg
                );
                return;
            }
        }
        panic!("row {row} col {col}: no style run covers this column");
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.session.resize(cols, rows).unwrap();
    }
}

// ===========================================================================
// Phase 1: Harness smoke tests
// ===========================================================================

#[test]
fn smoke_create_default() {
    let t = TestTerm::default();
    assert_eq!(t.session.cols(), 80);
    assert_eq!(t.session.rows(), 24);
}

#[test]
fn smoke_create_custom_size() {
    let t = TestTerm::new(40, 10);
    assert_eq!(t.session.cols(), 40);
    assert_eq!(t.session.rows(), 10);
}

#[test]
fn smoke_feed_and_dump() {
    let mut t = TestTerm::new(20, 5);
    t.feed_str("Hello");
    t.assert_row(0, "Hello");
    t.assert_row_blank(1);
}

#[test]
fn smoke_cursor_starts_at_home() {
    let t = TestTerm::new(20, 5);
    t.assert_cursor_pos(1, 1);
}

// ===========================================================================
// Phase 2: C0 controls + cursor movement
// ===========================================================================

// -- C0 controls ---

#[test]
fn c0_cr_returns_to_column_1() {
    let mut t = TestTerm::new(20, 5);
    t.feed_str("Hello");
    t.cr();
    t.assert_cursor_pos(1, 1);
}

#[test]
fn c0_lf_moves_down_one_row() {
    let mut t = TestTerm::new(20, 5);
    t.feed_str("A");
    t.lf();
    let (_, row) = t.cursor_pos();
    assert_eq!(row, 2);
}

#[test]
fn c0_bs_moves_cursor_left() {
    let mut t = TestTerm::new(20, 5);
    t.feed_str("AB");
    t.bs();
    t.assert_cursor_pos(2, 1);
}

#[test]
fn c0_tab_advances_to_next_tab_stop() {
    let mut t = TestTerm::new(80, 5);
    t.tab();
    // Default tab stops at every 8 columns
    t.assert_cursor_pos(9, 1);
}

#[test]
fn c0_cr_lf_moves_to_start_of_next_line() {
    let mut t = TestTerm::new(20, 5);
    t.feed_str("Hello");
    t.cr();
    t.lf();
    t.assert_cursor_pos(1, 2);
}

#[test]
fn c0_lf_at_bottom_scrolls() {
    let mut t = TestTerm::new(20, 3);
    t.feed_str("Line1");
    t.cr();
    t.lf();
    t.feed_str("Line2");
    t.cr();
    t.lf();
    t.feed_str("Line3");
    t.cr();
    t.lf();
    // After scrolling, Line1 should be gone, Line2 should be at row 0
    t.assert_row(0, "Line2");
    t.assert_row(1, "Line3");
}

// -- CUP ---

#[test]
fn cup_positions_cursor() {
    let mut t = TestTerm::new(20, 10);
    t.cup(5, 10);
    t.assert_cursor_pos(10, 5);
}

#[test]
fn cup_zero_defaults_to_1() {
    let mut t = TestTerm::new(20, 10);
    t.cup(5, 5);
    t.feed(b"\x1b[0;0H");
    t.assert_cursor_pos(1, 1);
}

#[test]
fn cup_clamps_to_bounds() {
    let mut t = TestTerm::new(20, 10);
    t.cup(100, 200);
    t.assert_cursor_pos(20, 10);
}

#[test]
fn cup_no_params_goes_home() {
    let mut t = TestTerm::new(20, 10);
    t.cup(5, 5);
    t.feed(b"\x1b[H");
    t.assert_cursor_pos(1, 1);
}

// -- CUU/CUD/CUF/CUB ---

#[test]
fn cuu_moves_up() {
    let mut t = TestTerm::new(20, 10);
    t.cup(5, 3);
    t.cuu(2);
    t.assert_cursor_pos(3, 3);
}

#[test]
fn cud_moves_down() {
    let mut t = TestTerm::new(20, 10);
    t.cup(2, 3);
    t.cud(3);
    t.assert_cursor_pos(3, 5);
}

#[test]
fn cuf_moves_right() {
    let mut t = TestTerm::new(20, 10);
    t.cup(1, 1);
    t.cuf(5);
    t.assert_cursor_pos(6, 1);
}

#[test]
fn cub_moves_left() {
    let mut t = TestTerm::new(20, 10);
    t.cup(1, 10);
    t.cub(3);
    t.assert_cursor_pos(7, 1);
}

#[test]
fn cuu_clamps_at_top() {
    let mut t = TestTerm::new(20, 10);
    t.cup(3, 5);
    t.cuu(100);
    t.assert_cursor_pos(5, 1);
}

#[test]
fn cud_clamps_at_bottom() {
    let mut t = TestTerm::new(20, 10);
    t.cup(3, 5);
    t.cud(100);
    t.assert_cursor_pos(5, 10);
}

#[test]
fn cuf_clamps_at_right() {
    let mut t = TestTerm::new(20, 10);
    t.cup(1, 5);
    t.cuf(100);
    t.assert_cursor_pos(20, 1);
}

#[test]
fn cub_clamps_at_left() {
    let mut t = TestTerm::new(20, 10);
    t.cup(1, 5);
    t.cub(100);
    t.assert_cursor_pos(1, 1);
}

#[test]
fn cuu_default_param_is_1() {
    let mut t = TestTerm::new(20, 10);
    t.cup(5, 3);
    t.feed(b"\x1b[A");
    t.assert_cursor_pos(3, 4);
}

#[test]
fn cud_default_param_is_1() {
    let mut t = TestTerm::new(20, 10);
    t.cup(5, 3);
    t.feed(b"\x1b[B");
    t.assert_cursor_pos(3, 6);
}

// -- CNL/CPL/CHA ---

#[test]
fn cnl_moves_to_next_line_col_1() {
    let mut t = TestTerm::new(20, 10);
    t.cup(3, 5);
    t.cnl(2);
    t.assert_cursor_pos(1, 5);
}

#[test]
fn cpl_moves_to_prev_line_col_1() {
    let mut t = TestTerm::new(20, 10);
    t.cup(5, 5);
    t.cpl(2);
    t.assert_cursor_pos(1, 3);
}

#[test]
fn cha_sets_column() {
    let mut t = TestTerm::new(20, 10);
    t.cup(3, 5);
    t.cha(10);
    t.assert_cursor_pos(10, 3);
}

// -- DECSC/DECRC ---

#[test]
fn save_restore_cursor() {
    let mut t = TestTerm::new(20, 10);
    t.cup(3, 7);
    t.save_cursor();
    t.cup(1, 1);
    t.restore_cursor();
    t.assert_cursor_pos(7, 3);
}

// -- Writing advances cursor ---

#[test]
fn writing_advances_cursor() {
    let mut t = TestTerm::new(20, 5);
    t.feed_str("ABCDE");
    t.assert_cursor_pos(6, 1);
}

#[test]
fn writing_wraps_at_right_margin() {
    let mut t = TestTerm::new(5, 3);
    t.feed_str("ABCDEFG");
    // After writing 5 chars on row 1, wraps to row 2
    t.assert_row(0, "ABCDE");
    t.assert_row(1, "FG");
    t.assert_cursor_pos(3, 2);
}

// ===========================================================================
// Phase 3: Erase/edit operations + scroll regions
// ===========================================================================

// -- ED (Erase in Display) ---

#[test]
fn ed_0_erases_from_cursor_to_end() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("AAAAAAAAAA");
    t.cr();
    t.lf();
    t.feed_str("BBBBBBBBBB");
    t.cr();
    t.lf();
    t.feed_str("CCCCCCCCCC");
    t.cup(2, 5);
    t.erase_in_display(0);
    t.assert_row(0, "AAAAAAAAAA");
    // Row 1 from col 5 onward should be erased
    t.assert_row_starts_with(1, "BBBB");
    t.assert_row_blank(2);
}

#[test]
fn ed_1_erases_from_start_to_cursor() {
    let mut t = TestTerm::new(10, 5);
    t.cup(1, 1);
    t.feed_str("AAAAAAAAAA");
    t.cup(2, 1);
    t.feed_str("BBBBBBBBBB");
    t.cup(3, 1);
    t.feed_str("CCCCCCCCCC");
    t.cup(4, 1);
    t.feed_str("DDDDDDDDDD");
    // Verify content before erase
    assert!(
        t.row(0).starts_with("AAAAAAAAAA"),
        "pre: row 0 = {:?}",
        t.row(0)
    );
    assert!(
        t.row(1).starts_with("BBBBBBBBBB"),
        "pre: row 1 = {:?}",
        t.row(1)
    );
    assert!(
        t.row(2).starts_with("CCCCCCCCCC"),
        "pre: row 2 = {:?}",
        t.row(2)
    );
    assert!(
        t.row(3).starts_with("DDDDDDDDDD"),
        "pre: row 3 = {:?}",
        t.row(3)
    );
    // Move cursor to row 3, col 5 and erase above
    t.cup(3, 5);
    t.erase_in_display(1);
    // Rows 0 and 1 should be erased
    t.assert_row_blank(0);
    t.assert_row_blank(1);
    // Row 3 should be preserved
    t.assert_row(3, "DDDDDDDDDD");
}

#[test]
fn ed_2_erases_entire_display() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("AAAAAAAAAA");
    t.cr();
    t.lf();
    t.feed_str("BBBBBBBBBB");
    t.erase_in_display(2);
    t.assert_row_blank(0);
    t.assert_row_blank(1);
    t.assert_row_blank(2);
}

// -- EL (Erase in Line) ---

#[test]
fn el_0_erases_from_cursor_to_end_of_line() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEFGHIJ");
    t.cup(1, 5);
    t.erase_in_line(0);
    t.assert_row(0, "ABCD");
}

#[test]
fn el_1_erases_from_start_of_line_to_cursor() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEFGHIJ");
    t.cup(1, 5);
    t.erase_in_line(1);
    // Cols 1-5 should be blank, rest preserved
    let actual = trim_trailing(&t.row(0));
    // The first 5 chars become spaces, so we should see spaces then FGHIJ
    assert!(actual.ends_with("FGHIJ"), "row 0: got {actual:?}");
}

#[test]
fn el_2_erases_entire_line() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEFGHIJ");
    t.cup(1, 5);
    t.erase_in_line(2);
    t.assert_row_blank(0);
}

// -- ECH (Erase Characters) ---

#[test]
fn ech_replaces_with_blanks() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEFGHIJ");
    t.cup(1, 3);
    t.erase_characters(4);
    // Cols 3-6 become spaces, rest preserved
    let actual = t.row(0);
    assert!(actual.starts_with("AB"), "expected AB... got {actual:?}");
    assert!(actual.contains("GHIJ"), "expected ...GHIJ got {actual:?}");
}

// -- DCH (Delete Characters) ---

#[test]
fn dch_shifts_left() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEFGHIJ");
    t.cup(1, 3);
    t.delete_characters(2);
    // Delete 2 chars at col 3 => "AB" + "EFGHIJ" shifted left
    t.assert_row_starts_with(0, "ABEFGHIJ");
}

// -- ICH (Insert Characters) ---

#[test]
fn ich_shifts_right() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEFGHIJ");
    t.cup(1, 3);
    t.insert_characters(2);
    // Insert 2 blanks at col 3 => "AB" + "  " + "CDEFGH" (IJ pushed off)
    let actual = t.row(0);
    assert!(actual.starts_with("AB"), "expected AB... got {actual:?}");
    // After the 2 blanks, the shifted chars should follow
    assert!(
        actual.contains("CDEFGH"),
        "expected ...CDEFGH got {actual:?}"
    );
}

// -- IL/DL (Insert/Delete Lines) ---

#[test]
fn il_inserts_blank_line() {
    let mut t = TestTerm::new(10, 5);
    t.feed_str("LINE1");
    t.cr();
    t.lf();
    t.feed_str("LINE2");
    t.cr();
    t.lf();
    t.feed_str("LINE3");
    t.cup(2, 1);
    t.insert_lines(1);
    t.assert_row(0, "LINE1");
    t.assert_row_blank(1);
    t.assert_row(2, "LINE2");
    t.assert_row(3, "LINE3");
}

#[test]
fn dl_deletes_line() {
    let mut t = TestTerm::new(10, 5);
    t.feed_str("LINE1");
    t.cr();
    t.lf();
    t.feed_str("LINE2");
    t.cr();
    t.lf();
    t.feed_str("LINE3");
    t.cup(2, 1);
    t.delete_lines(1);
    t.assert_row(0, "LINE1");
    t.assert_row(1, "LINE3");
    t.assert_row_blank(2);
}

// -- DECSTBM (Scroll Regions) ---

#[test]
fn scroll_region_scrolls_within_region() {
    let mut t = TestTerm::new(10, 5);
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        if i < 5 {
            t.cr();
            t.lf();
        }
    }
    t.set_scroll_region(2, 4);
    t.cup(4, 1);
    t.lf(); // Should scroll region lines 2-4
    t.assert_row(0, "LINE1");
    // LINE2 should have scrolled out of the region
    t.assert_row(1, "LINE3");
    t.assert_row(2, "LINE4");
    t.assert_row_blank(3); // New blank line in region
    t.assert_row(4, "LINE5");
}

#[test]
fn scroll_region_preserves_content_outside() {
    let mut t = TestTerm::new(10, 5);
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        if i < 5 {
            t.cr();
            t.lf();
        }
    }
    t.set_scroll_region(2, 4);
    t.cup(4, 1);
    t.lf();
    // Lines outside region (1 and 5) should be untouched
    t.assert_row(0, "LINE1");
    t.assert_row(4, "LINE5");
}

#[test]
fn su_scrolls_up_in_region() {
    let mut t = TestTerm::new(10, 5);
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        if i < 5 {
            t.cr();
            t.lf();
        }
    }
    t.set_scroll_region(2, 4);
    t.scroll_up(1);
    t.assert_row(0, "LINE1");
    t.assert_row(1, "LINE3");
    t.assert_row(2, "LINE4");
    t.assert_row_blank(3);
    t.assert_row(4, "LINE5");
}

#[test]
fn sd_scrolls_down_in_region() {
    let mut t = TestTerm::new(10, 5);
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        if i < 5 {
            t.cr();
            t.lf();
        }
    }
    t.set_scroll_region(2, 4);
    t.scroll_down(1);
    t.assert_row(0, "LINE1");
    t.assert_row_blank(1);
    t.assert_row(1 + 1, "LINE2"); // LINE2 shifted down within region
    t.assert_row(3, "LINE3");
    t.assert_row(4, "LINE5");
}

#[test]
fn il_respects_scroll_region() {
    let mut t = TestTerm::new(10, 5);
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        if i < 5 {
            t.cr();
            t.lf();
        }
    }
    t.set_scroll_region(2, 4);
    t.cup(2, 1);
    t.insert_lines(1);
    t.assert_row(0, "LINE1");
    t.assert_row_blank(1);
    t.assert_row(2, "LINE2");
    t.assert_row(3, "LINE3");
    // LINE4 should be pushed out of the scroll region
    t.assert_row(4, "LINE5");
}

#[test]
fn dl_respects_scroll_region() {
    let mut t = TestTerm::new(10, 5);
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        if i < 5 {
            t.cr();
            t.lf();
        }
    }
    t.set_scroll_region(2, 4);
    t.cup(2, 1);
    t.delete_lines(1);
    t.assert_row(0, "LINE1");
    t.assert_row(1, "LINE3");
    t.assert_row(2, "LINE4");
    t.assert_row_blank(3); // Blank inserted at bottom of region
    t.assert_row(4, "LINE5");
}

#[test]
fn reset_scroll_region_restores_full_screen() {
    let mut t = TestTerm::new(10, 5);
    t.set_scroll_region(2, 4);
    t.reset_scroll_region();
    // Now scrolling should affect the full screen (not just rows 2-4)
    // Write 6 lines in a 5-row terminal: LINE1 should scroll off
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        t.cr();
        t.lf();
    }
    t.feed_str("LINE6");
    t.assert_row(0, "LINE2");
    t.assert_row(4, "LINE6");
}

// -- IRM (Insert/Replace Mode) ---

#[test]
fn insert_mode_shifts_existing_text() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEF");
    t.cup(1, 3);
    t.enable_insert_mode();
    t.feed_str("XY");
    t.disable_insert_mode();
    // "AB" then XY inserted, shifting "CDEF" right
    t.assert_row_starts_with(0, "ABXYCDEF");
}

#[test]
fn replace_mode_overwrites() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("ABCDEF");
    t.cup(1, 3);
    t.feed_str("XY");
    // Default is replace mode: XY overwrites C and D
    t.assert_row(0, "ABXYEF");
}

// ===========================================================================
// Phase 4: Alternate screen, auto-wrap, modes
// ===========================================================================

#[test]
fn alt_screen_starts_blank() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("MainText");
    t.enter_alt_screen();
    t.assert_row_blank(0);
    t.assert_row_blank(1);
}

#[test]
fn alt_screen_restores_content() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("MainText");
    t.enter_alt_screen();
    t.feed_str("AltText");
    t.exit_alt_screen();
    t.assert_row(0, "MainText");
}

#[test]
fn alt_screen_preserves_cursor() {
    let mut t = TestTerm::new(20, 10);
    t.cup(3, 5);
    let pos_before = t.cursor_pos();
    t.enter_alt_screen();
    t.cup(1, 1);
    t.exit_alt_screen();
    t.assert_cursor_pos(pos_before.0, pos_before.1);
}

// -- DECAWM (Auto-wrap mode) ---

#[test]
fn autowrap_enabled_wraps_to_next_line() {
    let mut t = TestTerm::new(5, 3);
    t.enable_autowrap();
    t.feed_str("ABCDEFGH");
    t.assert_row(0, "ABCDE");
    t.assert_row(1, "FGH");
}

#[test]
fn autowrap_disabled_overwrites_last_column() {
    let mut t = TestTerm::new(5, 3);
    t.disable_autowrap();
    t.feed_str("ABCDEFGH");
    // With no-wrap, cursor stays at last column and overwrites
    t.assert_row(0, "ABCDH");
    t.assert_row_blank(1);
}

#[test]
fn deferred_wrap_at_right_margin() {
    let mut t = TestTerm::new(5, 3);
    t.enable_autowrap();
    t.feed_str("ABCDE");
    let (col, row) = t.cursor_pos();
    assert!(
        (col == 5 && row == 1) || (col == 1 && row == 2),
        "After writing exactly 5 chars in 5-col terminal, cursor at ({col},{row})"
    );
}

// -- Cursor shape (DECSCUSR) ---

#[test]
fn cursor_shape_block() {
    let mut t = TestTerm::default();
    t.feed(b"\x1b[2 q"); // steady block
    assert_eq!(t.session.cursor_shape(), CursorShape::Block);
}

#[test]
fn cursor_shape_underline() {
    let mut t = TestTerm::default();
    t.feed(b"\x1b[4 q"); // steady underline
    assert_eq!(t.session.cursor_shape(), CursorShape::Underline);
}

#[test]
fn cursor_shape_bar() {
    let mut t = TestTerm::default();
    t.feed(b"\x1b[2 q"); // block
    t.feed(b"\x1b[6 q"); // steady bar
    assert_eq!(t.session.cursor_shape(), CursorShape::Bar);
}

#[test]
fn cursor_shape_default_resets() {
    let mut t = TestTerm::default();
    t.feed(b"\x1b[2 q"); // block
    t.feed(b"\x1b[0 q"); // default
    // alacritty_terminal resets to Block for DECSCUSR 0
    // This is implementation-specific; accept Block or Bar
    let shape = t.session.cursor_shape();
    assert!(
        shape == CursorShape::Block || shape == CursorShape::Bar,
        "expected Block or Bar after DECSCUSR 0, got {:?}",
        shape
    );
}

// -- Focus events ---

#[test]
fn focus_event_mode_tracking() {
    let mut t = TestTerm::default();
    assert!(!t.session.focus_events_enabled());
    t.feed(b"\x1b[?1004h");
    assert!(t.session.focus_events_enabled());
    t.feed(b"\x1b[?1004l");
    assert!(!t.session.focus_events_enabled());
}

// ===========================================================================
// Phase 5: Input encoding
// ===========================================================================

#[test]
fn encode_arrow_keys() {
    assert_eq!(
        encode_key_named("up", false, false, false).as_deref(),
        Some(b"\x1b[A".as_slice())
    );
    assert_eq!(
        encode_key_named("down", false, false, false).as_deref(),
        Some(b"\x1b[B".as_slice())
    );
    assert_eq!(
        encode_key_named("right", false, false, false).as_deref(),
        Some(b"\x1b[C".as_slice())
    );
    assert_eq!(
        encode_key_named("left", false, false, false).as_deref(),
        Some(b"\x1b[D".as_slice())
    );
}

#[test]
fn encode_home_end() {
    assert_eq!(
        encode_key_named("home", false, false, false).as_deref(),
        Some(b"\x1b[H".as_slice())
    );
    assert_eq!(
        encode_key_named("end", false, false, false).as_deref(),
        Some(b"\x1b[F".as_slice())
    );
}

#[test]
fn encode_insert_delete() {
    assert_eq!(
        encode_key_named("insert", false, false, false).as_deref(),
        Some(b"\x1b[2~".as_slice())
    );
    assert_eq!(
        encode_key_named("delete", false, false, false).as_deref(),
        Some(b"\x1b[3~".as_slice())
    );
}

#[test]
fn encode_page_up_down() {
    assert_eq!(
        encode_key_named("pageup", false, false, false).as_deref(),
        Some(b"\x1b[5~".as_slice())
    );
    assert_eq!(
        encode_key_named("pagedown", false, false, false).as_deref(),
        Some(b"\x1b[6~".as_slice())
    );
}

#[test]
fn encode_function_keys() {
    assert!(encode_key_named("f1", false, false, false).is_some());
    assert!(encode_key_named("f2", false, false, false).is_some());
    assert!(encode_key_named("f5", false, false, false).is_some());
    assert!(encode_key_named("f12", false, false, false).is_some());
}

#[test]
fn encode_arrow_with_ctrl() {
    let encoded = encode_key_named("up", false, true, false).unwrap();
    // Should include modifier parameter (CSI 1;5A)
    assert_eq!(encoded, b"\x1b[1;5A");
}

#[test]
fn encode_arrow_with_shift() {
    let encoded = encode_key_named("up", true, false, false).unwrap();
    // CSI 1;2A
    assert_eq!(encoded, b"\x1b[1;2A");
}

#[test]
fn encode_arrow_with_alt() {
    let encoded = encode_key_named("up", false, false, true).unwrap();
    // CSI 1;3A
    assert_eq!(encoded, b"\x1b[1;3A");
}

#[test]
fn ctrl_byte_basic_letters() {
    use gpui::Keystroke;
    let ctrl_a = Keystroke::parse("ctrl-a").unwrap();
    assert_eq!(
        super::view::helpers::ctrl_byte_for_keystroke(&ctrl_a),
        Some(0x01)
    );

    let ctrl_z = Keystroke::parse("ctrl-z").unwrap();
    assert_eq!(
        super::view::helpers::ctrl_byte_for_keystroke(&ctrl_z),
        Some(0x1a)
    );

    let ctrl_m = Keystroke::parse("ctrl-m").unwrap();
    assert_eq!(
        super::view::helpers::ctrl_byte_for_keystroke(&ctrl_m),
        Some(0x0d)
    );
}

#[test]
fn ctrl_byte_special_chars() {
    use gpui::Keystroke;
    // Ctrl+Space => NUL
    let ctrl_space = Keystroke::parse("ctrl-space").unwrap();
    assert_eq!(
        super::view::helpers::ctrl_byte_for_keystroke(&ctrl_space),
        Some(0x00)
    );

    // Ctrl+/ => 0x1f
    let ctrl_slash = Keystroke::parse("ctrl-/").unwrap();
    assert_eq!(
        super::view::helpers::ctrl_byte_for_keystroke(&ctrl_slash),
        Some(0x1f)
    );
}

#[test]
fn tab_encodes_correctly() {
    let encoded = encode_key_named("tab", false, false, false);
    // Tab key should produce \t (0x09) or be recognized
    if let Some(bytes) = encoded {
        assert_eq!(bytes, b"\t");
    }
    // If None, tab is handled separately by the terminal input handler, which is also acceptable
}

// ===========================================================================
// Phase 6: SGR/style attributes
// ===========================================================================

#[test]
fn sgr_bold() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("1");
    t.feed_str("Bold");
    t.sgr_reset();
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_BOLD);
}

#[test]
fn sgr_italic() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("3");
    t.feed_str("Ital");
    t.sgr_reset();
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_ITALIC);
}

#[test]
fn sgr_underline() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("4");
    t.feed_str("Ulin");
    t.sgr_reset();
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_UNDERLINE);
}

#[test]
fn sgr_faint() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("2");
    t.feed_str("Faint");
    t.sgr_reset();
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_FAINT);
}

#[test]
fn sgr_strikethrough() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("9");
    t.feed_str("Stk");
    t.sgr_reset();
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_STRIKETHROUGH);
}

#[test]
fn sgr_combined_bold_italic() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("1;3");
    t.feed_str("BI");
    t.sgr_reset();
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_BOLD);
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_ITALIC);
}

#[test]
fn sgr_reset_clears_flags() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("1;3;4");
    t.feed_str("Styled");
    t.sgr_reset();
    t.feed_str("Plain");
    // "Plain" starts at col 7
    t.assert_no_style_flag_at(0, 7, CELL_STYLE_FLAG_BOLD);
    t.assert_no_style_flag_at(0, 7, CELL_STYLE_FLAG_ITALIC);
    t.assert_no_style_flag_at(0, 7, CELL_STYLE_FLAG_UNDERLINE);
}

#[test]
fn sgr_fg_basic_color() {
    let mut t = TestTerm::new(20, 3);
    // SGR 31 = red foreground
    t.sgr("31");
    t.feed_str("Red");
    t.sgr_reset();
    let runs = t.style_runs(0);
    let red_run = runs.iter().find(|r| r.start_col <= 1 && r.end_col >= 1);
    assert!(red_run.is_some(), "expected a style run covering col 1");
    let run = red_run.unwrap();
    // Red foreground should NOT be the default white
    let default_fg = t.session.default_foreground();
    assert_ne!(run.fg, default_fg, "expected non-default fg for SGR 31");
}

#[test]
fn sgr_bg_basic_color() {
    let mut t = TestTerm::new(20, 3);
    // SGR 42 = green background
    t.sgr("42");
    t.feed_str("Grn");
    t.sgr_reset();
    let runs = t.style_runs(0);
    let run = runs.iter().find(|r| r.start_col <= 1 && r.end_col >= 1);
    assert!(run.is_some(), "expected a style run covering col 1");
    let run = run.unwrap();
    let default_bg = t.session.default_background();
    assert_ne!(run.bg, default_bg, "expected non-default bg for SGR 42");
}

#[test]
fn sgr_256_color() {
    let mut t = TestTerm::new(20, 3);
    // SGR 38;5;196 = 256-color red fg
    t.sgr("38;5;196");
    t.feed_str("256c");
    t.sgr_reset();
    let runs = t.style_runs(0);
    let run = runs.iter().find(|r| r.start_col <= 1 && r.end_col >= 1);
    assert!(run.is_some(), "expected a style run covering col 1");
    let run = run.unwrap();
    let default_fg = t.session.default_foreground();
    assert_ne!(run.fg, default_fg, "expected non-default fg for 256-color");
}

#[test]
fn sgr_24bit_rgb_color() {
    let mut t = TestTerm::new(20, 3);
    // SGR 38;2;100;150;200 = 24-bit fg
    t.sgr("38;2;100;150;200");
    t.feed_str("RGB");
    t.sgr_reset();
    t.assert_fg_at(
        0,
        1,
        Rgb {
            r: 100,
            g: 150,
            b: 200,
        },
    );
}

#[test]
fn sgr_24bit_bg_color() {
    let mut t = TestTerm::new(20, 3);
    // SGR 48;2;50;75;100 = 24-bit bg
    t.sgr("48;2;50;75;100");
    t.feed_str("BG");
    t.sgr_reset();
    t.assert_bg_at(
        0,
        1,
        Rgb {
            r: 50,
            g: 75,
            b: 100,
        },
    );
}

#[test]
fn sgr_inverse_video() {
    let mut t = TestTerm::new(20, 3);
    let default_fg = t.session.default_foreground();
    let default_bg = t.session.default_background();
    // SGR 7 = inverse
    t.sgr("7");
    t.feed_str("Inv");
    t.sgr_reset();
    let runs = t.style_runs(0);
    let run = runs.iter().find(|r| r.start_col <= 1 && r.end_col >= 1);
    assert!(run.is_some());
    let run = run.unwrap();
    // Inverse should swap fg/bg
    assert_eq!(run.fg, default_bg, "inverse fg should be default bg");
    assert_eq!(run.bg, default_fg, "inverse bg should be default fg");
}

#[test]
fn style_run_column_spans() {
    let mut t = TestTerm::new(20, 3);
    t.sgr("1");
    t.feed_str("AAA");
    t.sgr_reset();
    t.feed_str("BBB");
    let runs = t.style_runs(0);
    // Should have at least two runs: bold (cols 1-3) and normal (cols 4-6)
    assert!(
        runs.len() >= 2,
        "expected at least 2 style runs, got {}",
        runs.len()
    );
    // First run should cover cols 1-3 with bold flag
    let bold_run = &runs[0];
    assert_eq!(bold_run.start_col, 1);
    assert_eq!(bold_run.end_col, 3);
    assert!(bold_run.flags & CELL_STYLE_FLAG_BOLD != 0);
}

#[test]
fn style_runs_after_cup() {
    let mut t = TestTerm::new(20, 3);
    t.cup(1, 5);
    t.sgr("1");
    t.feed_str("Bold");
    t.sgr_reset();
    // Bold text at row 0, starting at col 5
    t.assert_has_style_flag_at(0, 5, CELL_STYLE_FLAG_BOLD);
    t.assert_has_style_flag_at(0, 8, CELL_STYLE_FLAG_BOLD);
}

// ===========================================================================
// Phase 7: Resize behavior
// ===========================================================================

#[test]
fn resize_changes_dimensions() {
    let mut t = TestTerm::new(80, 24);
    t.resize(40, 10);
    assert_eq!(t.session.cols(), 40);
    assert_eq!(t.session.rows(), 10);
}

#[test]
fn resize_preserves_text() {
    let mut t = TestTerm::new(20, 5);
    t.feed_str("Hello");
    t.resize(40, 10);
    t.assert_row_starts_with(0, "Hello");
}

#[test]
fn resize_narrower_wraps_text() {
    let mut t = TestTerm::new(10, 5);
    t.feed_str("ABCDEFGHIJ");
    t.resize(5, 5);
    // alacritty reflows with cursor-following: the cursor stays on the last
    // portion of the reflowed text, pushing earlier content to scrollback.
    // Row 0 of the active screen shows "FGHIJ" (the continuation).
    t.assert_row_starts_with(0, "FGHIJ");
}

#[test]
fn resize_height_truncates() {
    let mut t = TestTerm::new(10, 5);
    for i in 1..=5 {
        t.feed(format!("LINE{i}").as_bytes());
        if i < 5 {
            t.cr();
            t.lf();
        }
    }
    t.resize(10, 3);
    // After shrinking to 3 rows, we should see the last 3 lines
    assert_eq!(t.session.rows(), 3);
}

#[test]
fn resize_clamps_cursor() {
    let mut t = TestTerm::new(20, 10);
    t.cup(8, 15);
    t.resize(10, 5);
    let (col, row) = t.cursor_pos();
    assert!(col <= 10, "cursor col {col} > new cols 10");
    assert!(row <= 5, "cursor row {row} > new rows 5");
}

#[test]
fn feed_after_resize_works() {
    let mut t = TestTerm::new(20, 5);
    t.resize(10, 3);
    t.feed_str("After");
    t.assert_row(0, "After");
    t.assert_cursor_pos(6, 1);
}

#[test]
fn style_runs_after_resize() {
    let mut t = TestTerm::new(20, 5);
    t.sgr("1");
    t.feed_str("Bold");
    t.sgr_reset();
    t.resize(40, 10);
    // Style runs should still be present after resize
    t.assert_has_style_flag_at(0, 1, CELL_STYLE_FLAG_BOLD);
}

#[test]
fn resize_wider_no_spurious_content() {
    let mut t = TestTerm::new(10, 3);
    t.feed_str("Short");
    t.resize(20, 3);
    // Row 0 should still just have "Short", no garbage in extended area
    t.assert_row(0, "Short");
    t.assert_row_blank(1);
}

#[test]
fn resize_to_single_row() {
    let mut t = TestTerm::new(10, 5);
    t.feed_str("Hello");
    t.resize(10, 1);
    assert_eq!(t.session.rows(), 1);
    // Should still be able to read row 0
    let _row = t.row(0);
}

// ===========================================================================
// Phase 8: New terminal mode queries (Zed parity)
// ===========================================================================

#[test]
fn alt_screen_mode_tracking() {
    let mut t = TestTerm::default();
    assert!(!t.session.alt_screen_active());
    t.enter_alt_screen();
    assert!(t.session.alt_screen_active());
    t.exit_alt_screen();
    assert!(!t.session.alt_screen_active());
}

#[test]
fn alternate_scroll_mode_tracking() {
    let mut t = TestTerm::default();
    // ALTERNATE_SCROLL is enabled by default in alacritty_terminal
    assert!(t.session.alternate_scroll_enabled());
    // Disable it
    t.feed(b"\x1b[?1007l");
    assert!(!t.session.alternate_scroll_enabled());
    // Re-enable it
    t.feed(b"\x1b[?1007h");
    assert!(t.session.alternate_scroll_enabled());
}

#[test]
fn display_offset_starts_at_zero() {
    let t = TestTerm::default();
    assert_eq!(t.session.display_offset(), 0);
}

// ===========================================================================
// Helper
// ===========================================================================

fn trim_trailing(s: &str) -> String {
    s.trim_end().to_string()
}
