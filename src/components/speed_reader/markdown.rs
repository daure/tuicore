use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

use super::{InlineMarks, ReaderToken};

#[derive(Debug, Clone, Copy)]
struct ListContext {
    next: Option<u64>,
    current: Option<u64>,
}

#[derive(Debug, Default)]
struct BlockBuilder {
    tokens: Vec<ReaderToken>,
    pending: Vec<(String, InlineMarks)>,
    block: usize,
    prefix: String,
    heading: bool,
    active: bool,
}

impl BlockBuilder {
    fn begin(&mut self, block: usize, prefix: String, heading: bool) {
        self.finish_word();
        self.block = block;
        self.prefix = prefix;
        self.heading = heading;
        self.active = true;
    }

    fn push_text(&mut self, text: &str, marks: InlineMarks) {
        let mut segment = String::new();
        for ch in text.chars() {
            if ch.is_whitespace() {
                self.push_segment(&mut segment, marks);
                self.finish_word();
            } else {
                segment.push(ch);
            }
        }
        self.push_segment(&mut segment, marks);
    }

    fn push_segment(&mut self, segment: &mut String, marks: InlineMarks) {
        if segment.is_empty() {
            return;
        }
        if let Some((text, previous_marks)) = self.pending.last_mut()
            && *previous_marks == marks
        {
            text.push_str(segment);
            segment.clear();
            return;
        }
        self.pending.push((std::mem::take(segment), marks));
    }

    fn finish_word(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let text = self
            .pending
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<String>();
        self.tokens.push(ReaderToken {
            text,
            fragments: std::mem::take(&mut self.pending),
            prefix: self.prefix.clone(),
            block: self.block,
            heading: self.heading,
            boundary_after: false,
        });
    }

    fn finish_block(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.finish_word();
        if let Some(token) = self.tokens.last_mut()
            && token.block == self.block
        {
            token.boundary_after = true;
        }
        self.active = false;
        true
    }
}

pub(super) fn parse_markdown(source: &str) -> Vec<ReaderToken> {
    let mut builder = BlockBuilder::default();
    let mut block = 0;
    let mut heading = None;
    let mut quote_depth = 0usize;
    let mut lists = Vec::<ListContext>::new();
    let mut marks = InlineMarks::default();
    let mut image_depth = 0usize;
    let mut code_block_depth = 0usize;

    for event in Parser::new(source) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(level);
                builder.begin(block, context_prefix(quote_depth, &lists, heading), true);
            }
            Event::End(TagEnd::Heading(_)) => {
                if builder.finish_block() {
                    block += 1;
                }
                heading = None;
            }
            Event::Start(Tag::Paragraph) => {
                builder.begin(block, context_prefix(quote_depth, &lists, heading), false);
            }
            Event::End(TagEnd::Paragraph) => {
                if builder.finish_block() {
                    block += 1;
                }
            }
            Event::Start(Tag::BlockQuote(_)) => quote_depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote_depth = quote_depth.saturating_sub(1),
            Event::Start(Tag::List(start)) => {
                if builder.finish_block() {
                    block += 1;
                }
                lists.push(ListContext {
                    next: start,
                    current: None,
                });
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                if let Some(list) = lists.last_mut() {
                    list.current = list.next;
                    if let Some(next) = &mut list.next {
                        *next += 1;
                    }
                }
            }
            Event::End(TagEnd::Item) => {
                if builder.finish_block() {
                    block += 1;
                }
                if let Some(list) = lists.last_mut() {
                    list.current = None;
                }
            }
            Event::Start(Tag::Emphasis) => marks.emphasis = true,
            Event::End(TagEnd::Emphasis) => marks.emphasis = false,
            Event::Start(Tag::Strong) => marks.strong = true,
            Event::End(TagEnd::Strong) => marks.strong = false,
            Event::Start(Tag::Link { .. }) => marks.link = true,
            Event::End(TagEnd::Link) => marks.link = false,
            Event::Start(Tag::Image { .. }) => image_depth += 1,
            Event::End(TagEnd::Image) => image_depth = image_depth.saturating_sub(1),
            Event::Start(Tag::CodeBlock(_)) => code_block_depth += 1,
            Event::End(TagEnd::CodeBlock) => code_block_depth = code_block_depth.saturating_sub(1),
            Event::Text(text) if image_depth == 0 && code_block_depth == 0 => {
                if !builder.active {
                    builder.begin(block, context_prefix(quote_depth, &lists, heading), false);
                }
                builder.push_text(&text, marks);
            }
            Event::SoftBreak | Event::HardBreak => builder.finish_word(),
            Event::Code(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::FootnoteReference(_)
            | Event::Rule
            | Event::TaskListMarker(_)
            | Event::Text(_)
            | Event::Start(_)
            | Event::End(_) => {}
        }
    }
    builder.finish_block();
    builder.tokens
}

fn context_prefix(
    quote_depth: usize,
    lists: &[ListContext],
    heading: Option<HeadingLevel>,
) -> String {
    let mut parts = vec![">".to_string(); quote_depth];
    for list in lists {
        parts.push(match list.current {
            Some(index) => format!("{index}."),
            None => "-".to_string(),
        });
    }
    if let Some(level) = heading {
        parts.push(
            match level {
                HeadingLevel::H1 => "#",
                HeadingLevel::H2 => "##",
                HeadingLevel::H3 => "###",
                HeadingLevel::H4 => "####",
                HeadingLevel::H5 => "#####",
                HeadingLevel::H6 => "######",
            }
            .to_string(),
        );
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_keeps_full_context_and_inline_styles() {
        let tokens =
            parse_markdown("> - ## Read **this now** and [follow me](https://example.com)");

        assert_eq!(
            tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            ["Read", "this", "now", "and", "follow", "me"]
        );
        assert_eq!(tokens[0].prefix, "> - ##");
        assert!(tokens[1].fragments[0].1.strong);
        assert!(tokens[4].fragments[0].1.link);
    }

    #[test]
    fn markdown_omits_destinations_images_and_code() {
        let tokens = parse_markdown("[Label](https://example.com) ![alt](image.png) `code` prose");
        assert_eq!(
            tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            ["Label", "prose"]
        );
    }

    #[test]
    fn style_boundaries_inside_word_stay_one_word() {
        let tokens = parse_markdown("foo**bar**");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].text, "foobar");
        assert_eq!(tokens[0].fragments.len(), 2);
    }

    #[test]
    fn nested_list_items_do_not_merge_words_across_block_boundaries() {
        let tokens = parse_markdown(
            "- Use h and l to revisit individual words.\n  - Use j and k to move between blocks.",
        );

        assert_eq!(
            tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            [
                "Use",
                "h",
                "and",
                "l",
                "to",
                "revisit",
                "individual",
                "words.",
                "Use",
                "j",
                "and",
                "k",
                "to",
                "move",
                "between",
                "blocks."
            ]
        );
    }
}
