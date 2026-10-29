use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub const HEADING_SEPARATOR: &str = " › ";

/// A passage stops growing at the first block that ends past this. `ask` hands
/// the model 1 200 characters of each passage and the vector sees about as much,
/// so text beyond it was found but never read: a section of 8 000 characters
/// answered only from its opening lines.
const SPLIT_AT: usize = 1_200;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub heading_path: String,
    /// Byte offsets into the raw file. Byte, not character: the reader slices
    /// the file with them, and a char index lands mid-codepoint on any accent.
    pub start: usize,
    pub end: usize,
    /// Plain text, markdown syntax stripped — what gets indexed and shown in
    /// snippets. The offsets above still point at the raw source.
    pub text: String,
}

/// Splits a note at its headings. Each chunk carries the trail of headings above
/// it, which is what a result shows instead of just the file name.
pub fn chunk_markdown(source: &str) -> Vec<Chunk> {
    let parser = Parser::new_ext(source, Options::all()).into_offset_iter();

    let mut chunks: Vec<Chunk> = Vec::new();
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut start = 0usize;
    let mut heading_path = String::new();
    let mut text = String::new();

    let mut in_heading = false;
    let mut heading_level = 0u8;
    let mut heading_text = String::new();
    // The heading this chunk opened with, to recognise a chunk that is nothing
    // but it.
    let mut opened_with = String::new();

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                push(&mut chunks, &heading_path, start, range.start, &text, &opened_with);
                text.clear();
                start = range.start;
                in_heading = true;
                heading_level = depth(level);
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                while stack.last().is_some_and(|(l, _)| *l >= heading_level) {
                    stack.pop();
                }
                stack.push((heading_level, heading_text.trim().to_string()));
                heading_path = stack
                    .iter()
                    .map(|(_, t)| t.as_str())
                    .collect::<Vec<_>>()
                    .join(HEADING_SEPARATOR);
                // The heading is part of its own chunk's searchable text.
                push_text(&mut text, &heading_text);
                opened_with = heading_text.trim().to_string();
            }
            Event::Text(t) | Event::Code(t) => {
                if in_heading {
                    heading_text.push_str(&t);
                } else {
                    push_text(&mut text, &t);
                }
            }
            Event::SoftBreak | Event::HardBreak => push_text(&mut text, "\n"),
            // Cut only where a block ends, so no sentence, list item or code
            // block is split in two; the next passage keeps the same trail.
            Event::End(TagEnd::Paragraph | TagEnd::Item | TagEnd::CodeBlock | TagEnd::Table)
                if text.chars().count() >= SPLIT_AT =>
            {
                push(&mut chunks, &heading_path, start, range.end, &text, &opened_with);
                text.clear();
                start = range.end;
                opened_with.clear();
            }
            _ => {}
        }
    }
    push(&mut chunks, &heading_path, start, source.len(), &text, &opened_with);
    chunks
}

/// Image destinations referenced by a note, used to tie a screenshot back to the
/// note it illustrates.
pub fn image_links(source: &str) -> Vec<String> {
    Parser::new_ext(source, Options::all())
        .filter_map(|event| match event {
            Event::Start(Tag::Image { dest_url, .. }) => Some(dest_url.to_string()),
            _ => None,
        })
        .collect()
}

/// First H1 if there is one, otherwise the file name — the same rule Obsidian
/// and friends use, so imported vaults look right without a frontmatter pass.
pub fn title_of(source: &str, fallback: &str) -> String {
    let mut in_h1 = false;
    let mut title = String::new();
    for event in Parser::new_ext(source, Options::all()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) if level == HeadingLevel::H1 => in_h1 = true,
            Event::Text(t) | Event::Code(t) if in_h1 => title.push_str(&t),
            Event::End(TagEnd::Heading(_)) if in_h1 => break,
            _ => {}
        }
    }
    let title = title.trim();
    if title.is_empty() {
        fallback.to_string()
    } else {
        title.to_string()
    }
}

fn depth(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn push_text(buffer: &mut String, piece: &str) {
    if !buffer.is_empty() && !buffer.ends_with(['\n', ' ']) {
        buffer.push(' ');
    }
    buffer.push_str(piece);
}

/// A heading with nothing under it is not a passage. Indexed, it became a
/// document of four words that bm25 ranks above real prose — 63 of the 671
/// chunks in a real vault were exactly this, and they were what the AI got
/// handed when it was asked where CSS lived. The note is still found by its
/// title, which the search now weighs on its own.
fn push(
    chunks: &mut Vec<Chunk>,
    heading_path: &str,
    start: usize,
    end: usize,
    text: &str,
    opened_with: &str,
) {
    let text = text.trim();
    if text.is_empty() || end <= start {
        return;
    }
    if !opened_with.is_empty() && text == opened_with {
        return;
    }
    chunks.push(Chunk {
        heading_path: heading_path.to_string(),
        start,
        end,
        text: text.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_headings_and_keeps_the_trail() {
        let src = "# Nota\n\ntopo\n\n## Cache\n\nrevalidar\n\n### Tags\n\ntag aqui\n";
        let chunks = chunk_markdown(src);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].heading_path, "Nota");
        assert_eq!(chunks[1].heading_path, "Nota › Cache");
        assert_eq!(chunks[2].heading_path, "Nota › Cache › Tags");
    }

    /// 63 of the 671 chunks in a real vault were exactly this, and bm25 ranked
    /// them above prose because they are short.
    #[test]
    fn a_heading_with_nothing_under_it_is_not_a_passage() {
        let src = "# My Brand (Print on demand):\n\n## Cultivation:\n\n## Tem corpo\n\numa frase aqui\n";
        let chunks = chunk_markdown(src);

        assert_eq!(chunks.len(), 1, "só o que tem corpo vira trecho");
        assert_eq!(chunks[0].heading_path, "My Brand (Print on demand): › Tem corpo");
    }

    #[test]
    fn offsets_are_byte_exact_through_accents() {
        let src = "# Configuração\n\nprimeiro\n\n## Ação\n\nsegundo\n";
        let chunks = chunk_markdown(src);
        // Slicing with the offsets must not panic and must land on the heading.
        assert!(src[chunks[1].start..chunks[1].end].starts_with("## Ação"));
    }

    /// The study plan was one section of 8 000 characters; its end was never read.
    #[test]
    fn a_long_section_is_cut_between_paragraphs_and_keeps_its_trail() {
        let paragraph = "uma linha de estudo com vídeo e link ".repeat(10);
        let src = format!("# Estudo\n\n{}", format!("{paragraph}\n\n").repeat(12));
        let chunks = chunk_markdown(&src);

        assert!(chunks.len() > 1, "a seção longa continuou num trecho só");
        for chunk in &chunks {
            assert_eq!(chunk.heading_path, "Estudo");
            assert!(chunk.text.chars().count() < SPLIT_AT + paragraph.chars().count());
        }
        // Back to back, the pieces are the whole note: nothing dropped at a cut.
        let joined: String = chunks.iter().map(|c| &src[c.start..c.end]).collect();
        assert_eq!(joined.trim(), src.trim(), "um pedaço do texto ficou de fora");
    }

    #[test]
    fn a_short_section_stays_whole() {
        let chunks = chunk_markdown("# Nota\n\num parágrafo\n\noutro parágrafo\n");
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn title_falls_back_to_the_file_name() {
        assert_eq!(title_of("sem titulo\n", "server-actions"), "server-actions");
        assert_eq!(title_of("# Server Actions\n", "x"), "Server Actions");
    }

    #[test]
    fn finds_image_references() {
        let src = "texto\n\n![print](_assets/print-01.png)\n";
        assert_eq!(image_links(src), vec!["_assets/print-01.png"]);
    }
}
