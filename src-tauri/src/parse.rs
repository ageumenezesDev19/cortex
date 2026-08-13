use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub const HEADING_SEPARATOR: &str = " › ";

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

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                push(&mut chunks, &heading_path, start, range.start, &text);
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
            }
            Event::Text(t) | Event::Code(t) => {
                if in_heading {
                    heading_text.push_str(&t);
                } else {
                    push_text(&mut text, &t);
                }
            }
            Event::SoftBreak | Event::HardBreak => push_text(&mut text, "\n"),
            _ => {}
        }
    }
    push(&mut chunks, &heading_path, start, source.len(), &text);
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

fn push(chunks: &mut Vec<Chunk>, heading_path: &str, start: usize, end: usize, text: &str) {
    let text = text.trim();
    if text.is_empty() || end <= start {
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

    #[test]
    fn offsets_are_byte_exact_through_accents() {
        let src = "# Configuração\n\nprimeiro\n\n## Ação\n\nsegundo\n";
        let chunks = chunk_markdown(src);
        // Slicing with the offsets must not panic and must land on the heading.
        assert!(src[chunks[1].start..chunks[1].end].starts_with("## Ação"));
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
