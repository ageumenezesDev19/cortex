use ego_tree::NodeRef;
use scraper::{Html, Node};

/// The Notes.app body turned into markdown, plus what was left behind.
#[derive(Debug, Default, PartialEq)]
pub struct Converted {
    pub markdown: String,
    /// Images and embedded files dropped in this pass; each one left a mark.
    pub attachments: usize,
}

/// Notes writes **one `<div>` per line** and `<div><br></div>` for a blank one.
/// A generic HTML-to-markdown converter treats every `div` as a paragraph and
/// gives back a wall of double-spaced garbage, which is why this walks the tree
/// against the vocabulary Notes actually emits.
pub fn to_markdown(html: &str) -> Converted {
    let document = Html::parse_fragment(html);
    let mut out = Out::default();

    for child in document.root_element().children() {
        walk(child, &mut out);
    }
    out.flush();

    Converted {
        markdown: tidy(&out.lines.join("\n")),
        attachments: out.attachments,
    }
}

#[derive(Default)]
struct Out {
    lines: Vec<String>,
    line: String,
    /// Nesting depth of `ul`/`ol`, so indentation follows the source.
    depth: usize,
    ordered: Vec<bool>,
    attachments: usize,
}

impl Out {
    fn push(&mut self, text: &str) {
        self.line.push_str(text);
    }

    fn flush(&mut self) {
        let line = self.line.trim_end().to_string();
        self.line.clear();
        self.lines.push(line);
    }

    /// Ends the current line only if something is on it.
    fn break_line(&mut self) {
        if !self.line.trim().is_empty() {
            self.flush();
        }
    }
}

fn walk(node: NodeRef<'_, Node>, out: &mut Out) {
    match node.value() {
        Node::Text(text) => out.push(&escape(&text.text)),
        Node::Element(element) => {
            let children = || node.children();
            match element.name() {
                "br" => out.flush(),
                "div" | "p" => {
                    out.break_line();
                    for child in children() {
                        walk(child, out);
                    }
                    out.break_line();
                }
                name @ ("h1" | "h2" | "h3" | "h4" | "h5" | "h6") => {
                    out.break_line();
                    let level = name[1..].parse::<usize>().unwrap_or(1);

                    // Rendered apart because a <br> inside the heading flushes
                    // the shared buffer, and Notes opens most notes with an
                    // empty <h1><br></h1> that would leave a lone `#` behind.
                    let text = nested(children(), out);
                    if !text.is_empty() {
                        out.push(&format!("{} {text}", "#".repeat(level)));
                        out.break_line();
                    }
                }
                "ul" | "ol" => {
                    out.break_line();
                    out.depth += 1;
                    out.ordered.push(element.name() == "ol");
                    for child in children() {
                        walk(child, out);
                    }
                    out.ordered.pop();
                    out.depth -= 1;
                    out.break_line();
                }
                "li" => {
                    out.break_line();
                    let indent = "  ".repeat(out.depth.saturating_sub(1));
                    let marker = if *out.ordered.last().unwrap_or(&false) { "1." } else { "-" };
                    out.push(&format!("{indent}{marker} "));
                    for child in children() {
                        walk(child, out);
                    }
                    out.break_line();
                }
                "b" | "strong" => wrap(node, out, "**"),
                "i" | "em" => wrap(node, out, "*"),
                // Markdown has no underline; emphasis is the closest honest thing.
                "u" => wrap(node, out, "*"),
                "tt" | "code" | "samp" | "kbd" => wrap(node, out, "`"),
                "a" => {
                    let href = element.attr("href").unwrap_or_default().to_string();
                    let start = out.line.len();
                    for child in children() {
                        walk(child, out);
                    }
                    if !href.is_empty() {
                        let text: String = out.line[start..].to_string();
                        out.line.truncate(start);
                        out.push(&format!("[{text}]({href})"));
                    }
                }
                "img" | "object" | "embed" => {
                    out.attachments += 1;
                    out.break_line();
                    out.push(&format!("> ⟨imagem {} — ainda no Notes.app⟩", out.attachments));
                    out.flush();
                }
                "table" => table(node, out),
                // Style-only wrappers carry nothing worth keeping.
                _ => {
                    for child in children() {
                        walk(child, out);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Renders children into their own buffer and returns them as one line, keeping
/// the attachment tally with the caller.
fn nested<'a>(children: impl Iterator<Item = NodeRef<'a, Node>>, out: &mut Out) -> String {
    let mut inner = Out::default();
    for child in children {
        walk(child, &mut inner);
    }
    inner.flush();
    out.attachments += inner.attachments;
    inner.lines.join(" ").trim().to_string()
}

/// Applies an inline mark around whatever the children produced.
///
/// The children may contain a `<br>`, which ends the current line mid-mark — so
/// this cannot simply slice the live buffer afterwards, and did panic on real
/// notes when it tried. A mark that spans a break is closed and reopened on each
/// line, because `**` straddling a newline is not emphasis, it is two asterisks.
fn wrap(node: NodeRef<'_, Node>, out: &mut Out, fence: &str) {
    let start = out.line.len();
    let flushed_before = out.lines.len();

    for child in node.children() {
        walk(child, out);
    }

    let spanned = out.lines.len() > flushed_before;
    if !spanned {
        // An empty <b> around a <br> is common in Notes; `****` would be litter.
        if out.line[start..].trim().is_empty() {
            return;
        }
        // Notes splits one bold line into several adjacent <b> elements. Closing
        // and reopening would write `****`, which is not emphasis — so an
        // adjacent run is extended instead.
        if closes_with(&out.line[..start], fence) {
            // Remove the closing fence in place; truncating here would take the
            // text the children just appended with it.
            out.line.replace_range(start - fence.len()..start, "");
        } else {
            out.line.insert_str(start, fence);
        }
        out.push(fence);
        return;
    }

    for (index, line) in out.lines.iter_mut().enumerate().skip(flushed_before) {
        // Only the tail of the line we were already on belongs to this mark.
        let from = if index == flushed_before { start.min(line.len()) } else { 0 };
        if line[from..].trim().is_empty() {
            continue;
        }
        line.push_str(fence);
        line.insert_str(from, fence);
    }
    if !out.line.trim().is_empty() {
        out.line.insert_str(0, fence);
        out.push(fence);
    }
}

/// Whether the text ends with exactly one run of this fence, so `*` is never
/// mistaken for the tail of a `**`.
fn closes_with(text: &str, fence: &str) -> bool {
    let Some(rest) = text.strip_suffix(fence) else {
        return false;
    };
    let mark = fence.chars().next().unwrap_or('*');
    !rest.ends_with(mark)
}

fn table(node: NodeRef<'_, Node>, out: &mut Out) {
    let mut rows: Vec<Vec<String>> = Vec::new();
    collect_rows(node, &mut rows);
    if rows.is_empty() {
        return;
    }

    out.break_line();
    out.flush();

    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    for (index, row) in rows.iter().enumerate() {
        let mut cells = row.clone();
        cells.resize(width, String::new());
        out.lines.push(format!("| {} |", cells.join(" | ")));

        // GFM needs the delimiter row, and the first row becomes the header.
        if index == 0 {
            out.lines.push(format!("|{}", " --- |".repeat(width)));
        }
    }
    out.lines.push(String::new());
}

fn collect_rows(node: NodeRef<'_, Node>, rows: &mut Vec<Vec<String>>) {
    for child in node.children() {
        let Node::Element(element) = child.value() else { continue };
        match element.name() {
            "tr" => {
                let mut cells = Vec::new();
                for cell in child.children() {
                    if let Node::Element(inner) = cell.value() {
                        if inner.name() == "td" || inner.name() == "th" {
                            let mut inner = Out::default();
                            for content in cell.children() {
                                walk(content, &mut inner);
                            }
                            inner.flush();
                            // A cell is one line: a pipe would break the row.
                            cells.push(inner.lines.join(" ").trim().replace('|', "\\|"));
                        }
                    }
                }
                rows.push(cells);
            }
            _ => collect_rows(child, rows),
        }
    }
}

/// Notes leaves `&nbsp;` everywhere, and markdown reads it as a literal.
fn escape(text: &str) -> String {
    text.replace('\u{a0}', " ")
}

/// Notes emits a blank `<div>` between every line; three or more blank lines in
/// a row say nothing that one blank line does not.
fn tidy(markdown: &str) -> String {
    let mut out = String::new();
    let mut blanks = 0;

    for line in markdown.lines() {
        if line.trim().is_empty() {
            blanks += 1;
            if blanks > 1 {
                continue;
            }
        } else {
            blanks = 0;
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    format!("{}\n", out.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(html: &str) -> String {
        to_markdown(html).markdown
    }

    #[test]
    fn one_div_per_line_is_one_line_not_one_paragraph() {
        assert_eq!(md("<div>uma</div><div>outra</div>"), "uma\noutra\n");
    }

    #[test]
    fn an_empty_div_is_a_blank_line() {
        assert_eq!(md("<div>uma</div><div><br></div><div>outra</div>"), "uma\n\noutra\n");
    }

    #[test]
    fn headings_carry_their_level() {
        assert_eq!(md("<div><h1>Título</h1></div><div>corpo</div>"), "# Título\ncorpo\n");
        assert_eq!(md("<h2>Sub</h2>"), "## Sub\n");
    }

    #[test]
    fn the_empty_heading_notes_opens_with_is_dropped() {
        assert_eq!(md("<div><h1><br></h1></div><div>corpo</div>"), "corpo\n");
    }

    #[test]
    fn inline_marks_survive() {
        assert_eq!(md("<div><b>forte</b> e <i>leve</i></div>"), "**forte** e *leve*\n");
        assert_eq!(md("<div><tt>codigo</tt></div>"), "`codigo`\n");
    }

    #[test]
    fn an_empty_bold_around_a_break_leaves_no_litter() {
        // No stray `****`, and no blank line opening the note either.
        assert_eq!(md("<div><b><br></b></div><div>depois</div>"), "depois\n");
    }

    #[test]
    fn adjacent_bold_runs_become_one() {
        // Notes really does emit <b>a</b><b>b</b> for a single bold line.
        assert_eq!(md("<div><b>uma</b><b> linha</b></div>"), "**uma linha**\n");
    }

    #[test]
    fn an_italic_next_to_a_bold_is_not_merged() {
        assert_eq!(md("<div><b>forte</b><i>leve</i></div>"), "**forte***leve*\n");
    }

    #[test]
    fn lists_keep_their_nesting() {
        let html = "<ul><li>um</li><li>dois<ul><li>dois.um</li></ul></li></ul>";
        assert_eq!(md(html), "- um\n- dois\n  - dois.um\n");
    }

    #[test]
    fn ordered_lists_use_numbers() {
        assert_eq!(md("<ol><li>um</li><li>dois</li></ol>"), "1. um\n1. dois\n");
    }

    #[test]
    fn a_table_becomes_gfm_with_a_delimiter_row() {
        let html = "<table><tbody>\
            <tr><td><div>tecla</div></td><td><div>faz</div></td></tr>\
            <tr><td><div>Cmd+E</div></td><td><div>alterna</div></td></tr>\
            </tbody></table>";
        assert_eq!(
            md(html),
            "| tecla | faz |\n| --- | --- |\n| Cmd+E | alterna |\n"
        );
    }

    #[test]
    fn a_pipe_inside_a_cell_is_escaped() {
        let html = "<table><tbody><tr><td>a|b</td></tr></tbody></table>";
        assert!(md(html).contains("a\\|b"));
    }

    #[test]
    fn links_keep_their_destination() {
        assert_eq!(md(r#"<div><a href="https://x.dev">sítio</a></div>"#), "[sítio](https://x.dev)\n");
    }

    #[test]
    fn every_image_leaves_a_mark_and_is_counted() {
        let converted = to_markdown(
            "<div>antes</div><div><img src=\"data:image/png;base64,AAA\"></div><div>depois</div>",
        );
        assert_eq!(converted.attachments, 1);
        assert!(converted.markdown.contains("⟨imagem 1 — ainda no Notes.app⟩"));
        assert!(converted.markdown.contains("antes") && converted.markdown.contains("depois"));
    }

    #[test]
    fn style_only_wrappers_are_dropped() {
        assert_eq!(md("<div><span style=\"color:red\"><font>texto</font></span></div>"), "texto\n");
    }

    #[test]
    fn runs_of_blank_lines_collapse() {
        let html = "<div>um</div><div><br></div><div><br></div><div><br></div><div>dois</div>";
        assert_eq!(md(html), "um\n\ndois\n");
    }
}