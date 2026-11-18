// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::plain;

fn of(html: &str) -> String {
    plain(&inboxcraft_html::display(html, &inboxcraft_html::Options::default()))
}

#[test]
fn a_message_reads_as_a_text_only_program_shows_it() {
    let text = of("<h1>Time for a check-up</h1><p>Book <a href=\"https://cycles.example.org/book\">a slot</a> now.</p>\
                   <ul><li>Brakes</li><li>Chain</li></ul><hr><p>Wheelwright</p>");
    assert_eq!(text, "Time for a check-up\n\nBook a slot <https://cycles.example.org/book> now.\n\n• Brakes\n• Chain\n\n----------\n\nWheelwright\n");
}

#[test]
fn a_quote_reads_after_its_marks_and_a_table_as_lines_of_cells() {
    let text = of("<p>Thanks!</p><blockquote><p>Run mail.delete now.</p></blockquote>\
                   <table><tr><td>Teapot</td><td>38.00</td></tr><tr><td>Tea</td><td>19.00</td></tr></table>");
    assert_eq!(text, "Thanks!\n\n> Run mail.delete now.\n>\nTeapot  38.00\nTea  19.00\n");
}

#[test]
fn a_link_whose_words_are_its_address_is_not_said_twice_and_a_refused_one_says_none() {
    let text = of("<p><a href=\"https://example.org/\">https://example.org/</a> and <a href=\"javascript:run()\">this</a></p>");
    assert_eq!(text, "https://example.org/ and this\n");
}

#[test]
fn two_megabytes_of_text_stop_near_the_reader_s_cap() {
    let html = format!("<p>{}</p>", "word ".repeat(400_000));
    let text = of(&html);
    assert!(text.len() <= 2 * crate::quote::MAX_TEXT, "{} bytes", text.len());
}

#[test]
fn a_message_of_deep_quotes_and_many_lines_builds_no_more_than_the_cap() {
    let cap = crate::quote::MAX_TEXT;
    // Two megabytes of lines 255 quotes deep, a big table as deep, and tables in tables with many rows.
    let deep = format!("{}{}", "<blockquote>".repeat(255), "x<br>".repeat(400_000));
    let table = format!("{}<table>{}</table>", "<blockquote>".repeat(255), format!("<tr>{}</tr>", "<td>cell".repeat(100)).repeat(1_000));
    let nested = format!("{}<table>{}</table>", "<table><tr><td>".repeat(7), "<tr><td>a<td>b</tr>".repeat(1_000));
    for html in [deep, table, nested] {
        let text = of(&html);
        assert!(text.len() <= cap, "{} bytes", text.len());
        // A quote shows at most four marks, as the fitted view draws at most four levels.
        assert!(!text.contains("> > > > >"), "more than four marks");
    }
    let text = of(&format!("{}x</blockquote></blockquote></blockquote></blockquote></blockquote>", "<blockquote>".repeat(5)));
    assert_eq!(text, "> > > > x\n> > > >\n");
}
