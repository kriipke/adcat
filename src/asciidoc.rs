// Copyright 2025
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Convert AsciiDoc AST to pulldown-cmark events.

use acdc_parser::{
    Block, DelimitedBlockType, Document, InlineMacro, InlineNode, ListItem, OrderedList,
    Paragraph, Section, UnorderedList,
};
use pulldown_cmark::{CowStr, Event, HeadingLevel, LinkType, Tag, TagEnd};

/// Convert an AsciiDoc document to pulldown-cmark events.
pub fn document_to_events(doc: &Document) -> Vec<Event<'static>> {
    let mut events: Vec<Event<'static>> = Vec::new();

    if let Some(header) = &doc.header {
        if !header.title.is_empty() {
            events.extend(title_to_events(&header.title, HeadingLevel::H1));
        }
        if let Some(subtitle) = &header.subtitle {
            if !subtitle.is_empty() {
                events.extend(title_to_events(subtitle, HeadingLevel::H2));
            }
        }
    }

    for block in &doc.blocks {
        events.extend(block_to_events(block));
    }

    events
}

fn title_to_events(title: &acdc_parser::Title, level: HeadingLevel) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    events.push(Event::Start(Tag::Heading {
        level,
        id: None,
        classes: vec![],
        attrs: vec![],
    }));
    events.extend(inlines_to_events(title.as_ref()));
    events.push(Event::End(TagEnd::Heading(level)));
    events
}

fn section_to_events(section: &Section) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    let level = match section.level {
        0 => HeadingLevel::H1,
        1 => HeadingLevel::H1,
        2 => HeadingLevel::H2,
        3 => HeadingLevel::H3,
        4 => HeadingLevel::H4,
        5 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    };
    events.extend(title_to_events(&section.title, level));

    for block in &section.content {
        events.extend(block_to_events(block));
    }

    events
}

fn block_to_events(block: &Block) -> Vec<Event<'static>> {
    match block {
        Block::Paragraph(para) => paragraph_to_events(para),
        Block::Section(section) => section_to_events(section),
        Block::UnorderedList(list) => unordered_list_to_events(list),
        Block::OrderedList(list) => ordered_list_to_events(list),
        Block::DelimitedBlock(delimited) => delimited_block_to_events(delimited),
        Block::ThematicBreak(_) => vec![Event::Rule],
        Block::DiscreteHeader(header) => {
            let level = match header.level {
                0 => HeadingLevel::H1,
                1 => HeadingLevel::H1,
                2 => HeadingLevel::H2,
                3 => HeadingLevel::H3,
                4 => HeadingLevel::H4,
                5 => HeadingLevel::H5,
                _ => HeadingLevel::H6,
            };
            title_to_events(&header.title, level)
        }
        Block::Image(img) => {
            let dest = source_to_cowstr(&img.source);
            let alt = if img.title.is_empty() {
                CowStr::from("")
            } else {
                inlines_to_string(img.title.as_ref())
            };
            vec![
                Event::Start(Tag::Paragraph),
                Event::Start(Tag::Image {
                    link_type: LinkType::Inline,
                    dest_url: dest,
                    title: alt.clone(),
                    id: CowStr::from(""),
                }),
                Event::End(TagEnd::Image),
                Event::End(TagEnd::Paragraph),
            ]
        }
        Block::Admonition(admon) => {
            let label: &str = match admon.variant {
                acdc_parser::AdmonitionVariant::Note => "NOTE: ",
                acdc_parser::AdmonitionVariant::Tip => "TIP: ",
                acdc_parser::AdmonitionVariant::Important => "IMPORTANT: ",
                acdc_parser::AdmonitionVariant::Caution => "CAUTION: ",
                acdc_parser::AdmonitionVariant::Warning => "WARNING: ",
            };
            let mut events = vec![Event::Start(Tag::Paragraph)];
            events.push(Event::Text(label.to_string().into()));
            events.push(Event::End(TagEnd::Paragraph));
            for block in &admon.blocks {
                events.extend(block_to_events(block));
            }
            events
        }
        Block::Comment(_) => vec![],
        _ => vec![
            Event::Start(Tag::Paragraph),
            Event::Text(CowStr::from("[unsupported block]")),
            Event::End(TagEnd::Paragraph),
        ],
    }
}

fn paragraph_to_events(para: &Paragraph) -> Vec<Event<'static>> {
    vec![
        Event::Start(Tag::Paragraph),
        Event::Text(inlines_to_string(&para.content)),
        Event::End(TagEnd::Paragraph),
    ]
}

fn list_item_to_events(item: &ListItem) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::Item)];
    events.push(Event::Text(inlines_to_string(&item.principal)));
    for block in &item.blocks {
        events.extend(block_to_events(block));
    }
    events.push(Event::End(TagEnd::Item));
    events
}

fn unordered_list_to_events(list: &UnorderedList) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::List(None))];
    for item in &list.items {
        events.extend(list_item_to_events(item));
    }
    events.push(Event::End(TagEnd::List(false)));
    events
}

fn ordered_list_to_events(list: &OrderedList) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::List(Some(1)))];
    for item in &list.items {
        events.extend(list_item_to_events(item));
    }
    events.push(Event::End(TagEnd::List(true)));
    events
}

fn delimited_block_to_events(block: &acdc_parser::DelimitedBlock) -> Vec<Event<'static>> {
    match &block.inner {
        DelimitedBlockType::DelimitedListing(inlines)
        | DelimitedBlockType::DelimitedLiteral(inlines) => {
            let mut text = inlines_to_string(inlines).into_string();
            if !text.ends_with('\n') {
                text.push('\n');
            }
            vec![
                Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(
                    CowStr::from(""),
                ))),
                Event::Text(text.into()),
                Event::End(TagEnd::CodeBlock),
            ]
        }
        DelimitedBlockType::DelimitedQuote(blocks) => {
            let mut events = vec![Event::Start(Tag::BlockQuote(None))];
            for block in blocks {
                events.extend(block_to_events(block));
            }
            events.push(Event::End(TagEnd::BlockQuote(None)));
            events
        }
        DelimitedBlockType::DelimitedExample(blocks)
        | DelimitedBlockType::DelimitedSidebar(blocks)
        | DelimitedBlockType::DelimitedOpen(blocks) => {
            let mut events = vec![Event::Start(Tag::Paragraph)];
            for block in blocks {
                events.extend(block_to_events(block));
            }
            events.push(Event::End(TagEnd::Paragraph));
            events
        }
        DelimitedBlockType::DelimitedComment(_)
        | DelimitedBlockType::DelimitedPass(_)
        | DelimitedBlockType::DelimitedVerse(_)
        | DelimitedBlockType::DelimitedTable(_)
        | DelimitedBlockType::DelimitedStem(_)
        | _ => vec![],
    }
}

fn inlines_to_events(inlines: &[InlineNode]) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    for inline in inlines {
        events.extend(inline_to_events(inline));
    }
    events
}

fn inline_to_events(inline: &InlineNode) -> Vec<Event<'static>> {
    match inline {
        InlineNode::PlainText(plain) => {
            vec![Event::Text(plain.content.to_string().into())]
        }
        InlineNode::RawText(raw) => {
            vec![Event::Text(raw.content.to_string().into())]
        }
        InlineNode::VerbatimText(verbatim) => {
            vec![Event::Code(verbatim.content.to_string().into())]
        }
        InlineNode::BoldText(bold) => {
            let mut events = vec![Event::Start(Tag::Strong)];
            events.extend(inlines_to_events(&bold.content));
            events.push(Event::End(TagEnd::Strong));
            events
        }
        InlineNode::ItalicText(italic) => {
            let mut events = vec![Event::Start(Tag::Emphasis)];
            events.extend(inlines_to_events(&italic.content));
            events.push(Event::End(TagEnd::Emphasis));
            events
        }
        InlineNode::MonospaceText(mono) => {
            vec![Event::Code(inlines_to_string(&mono.content))]
        }
        InlineNode::HighlightText(highlight) => {
            let mut events = vec![Event::Start(Tag::Emphasis)];
            events.extend(inlines_to_events(&highlight.content));
            events.push(Event::End(TagEnd::Emphasis));
            events
        }
        InlineNode::SubscriptText(sub) => {
            let mut events = vec![Event::Text(CowStr::from("~"))];
            events.extend(inlines_to_events(&sub.content));
            events.push(Event::Text(CowStr::from("~")));
            events
        }
        InlineNode::SuperscriptText(sup) => {
            let mut events = vec![Event::Text(CowStr::from("^"))];
            events.extend(inlines_to_events(&sup.content));
            events.push(Event::Text(CowStr::from("^")));
            events
        }
        InlineNode::CurvedQuotationText(quote) => {
            let mut events = vec![Event::Text(CowStr::from("\u{201c}"))];
            events.extend(inlines_to_events(&quote.content));
            events.push(Event::Text(CowStr::from("\u{201d}")));
            events
        }
        InlineNode::CurvedApostropheText(_) => {
            vec![Event::Text(CowStr::from("\u{2019}"))]
        }
        InlineNode::StandaloneCurvedApostrophe(_) => {
            vec![Event::Text(CowStr::from("\u{2019}"))]
        }
        InlineNode::LineBreak(_) => vec![Event::SoftBreak],
        InlineNode::Macro(m) => macro_to_events(m),
        InlineNode::InlineAnchor(anchor) => {
            vec![Event::InlineHtml(format!(
                "<a id=\"{}\"></a>",
                html_escape(anchor.id)
            ).into())]
        }
        InlineNode::CalloutRef(callout) => {
            vec![Event::Text(format!("<{}>", callout.number).into())]
        }
        _ => vec![],
    }
}

fn macro_to_events(macro_node: &InlineMacro) -> Vec<Event<'static>> {
    match macro_node {
        InlineMacro::Link(link) => {
            let dest = source_to_cowstr(&link.target);
            let text = if link.text.is_empty() {
                source_to_cowstr(&link.target)
            } else {
                inlines_to_string(&link.text)
            };
            vec![
                Event::Start(Tag::Link {
                    link_type: LinkType::Inline,
                    dest_url: dest,
                    title: CowStr::from(""),
                    id: CowStr::from(""),
                }),
                Event::Text(text),
                Event::End(TagEnd::Link),
            ]
        }
        InlineMacro::Url(url) => {
            let dest = source_to_cowstr(&url.target);
            let text = if url.text.is_empty() {
                source_to_cowstr(&url.target)
            } else {
                inlines_to_string(&url.text)
            };
            vec![
                Event::Start(Tag::Link {
                    link_type: LinkType::Inline,
                    dest_url: dest,
                    title: CowStr::from(""),
                    id: CowStr::from(""),
                }),
                Event::Text(text),
                Event::End(TagEnd::Link),
            ]
        }
        InlineMacro::Autolink(autolink) => {
            let dest = source_to_cowstr(&autolink.url);
            vec![
                Event::Start(Tag::Link {
                    link_type: LinkType::Autolink,
                    dest_url: dest.clone(),
                    title: CowStr::from(""),
                    id: CowStr::from(""),
                }),
                Event::Text(dest),
                Event::End(TagEnd::Link),
            ]
        }
        InlineMacro::Image(img) => {
            let dest = source_to_cowstr(&img.source);
            let alt = if img.title.is_empty() {
                CowStr::from("")
            } else {
                inlines_to_string(img.title.as_ref())
            };
            vec![
                Event::Start(Tag::Image {
                    link_type: LinkType::Inline,
                    dest_url: dest,
                    title: alt,
                    id: CowStr::from(""),
                }),
                Event::End(TagEnd::Image),
            ]
        }
        InlineMacro::Footnote(footnote) => {
            let id = footnote.id.map(|s| s.to_string()).unwrap_or_default();
            let mut events = vec![Event::Start(Tag::Emphasis)];
            events.push(Event::Text(format!("[{}]", id).into()));
            events.push(Event::End(TagEnd::Emphasis));
            events
        }
        InlineMacro::Mailto(mailto) => {
            let dest = source_to_cowstr(&mailto.target);
            let text = if mailto.text.is_empty() {
                source_to_cowstr(&mailto.target)
            } else {
                inlines_to_string(&mailto.text)
            };
            vec![
                Event::Start(Tag::Link {
                    link_type: LinkType::Email,
                    dest_url: dest,
                    title: CowStr::from(""),
                    id: CowStr::from(""),
                }),
                Event::Text(text),
                Event::End(TagEnd::Link),
            ]
        }
        InlineMacro::CrossReference(xref) => {
            let text = if xref.text.is_empty() {
                xref.target.to_string().into()
            } else {
                inlines_to_string(xref.text.as_ref())
            };
            vec![
                Event::Start(Tag::Link {
                    link_type: LinkType::Inline,
                    dest_url: format!("#{}", xref.target).into(),
                    title: CowStr::from(""),
                    id: CowStr::from(""),
                }),
                Event::Text(text),
                Event::End(TagEnd::Link),
            ]
        }
        _ => vec![Event::Text(CowStr::from("[macro]"))],
    }
}

fn source_to_cowstr(source: &acdc_parser::Source) -> CowStr<'static> {
    source.to_string().into()
}

fn inlines_to_string(inlines: &[InlineNode]) -> CowStr<'static> {
    let mut result = String::new();
    for inline in inlines {
        match inline {
            InlineNode::PlainText(p) => result.push_str(p.content),
            InlineNode::RawText(r) => result.push_str(r.content),
            InlineNode::VerbatimText(v) => result.push_str(v.content),
            InlineNode::BoldText(b) => {
                result.push_str(&inlines_to_string(&b.content));
            }
            InlineNode::ItalicText(i) => {
                result.push_str(&inlines_to_string(&i.content));
            }
            InlineNode::MonospaceText(m) => {
                result.push_str(&inlines_to_string(&m.content));
            }
            InlineNode::LineBreak(_) => result.push('\n'),
            InlineNode::Macro(m) => {
                result.push_str(&macro_to_string(m));
            }
            _ => {}
        }
    }
    result.into()
}

fn macro_to_string(m: &InlineMacro) -> String {
    match m {
        InlineMacro::Link(l) => {
            let text = if l.text.is_empty() {
                l.target.to_string()
            } else {
                inlines_to_string(&l.text).into_string()
            };
            format!("{} ({})", text, l.target)
        }
        InlineMacro::Url(u) => {
            let text = if u.text.is_empty() {
                u.target.to_string()
            } else {
                inlines_to_string(&u.text).into_string()
            };
            format!("{} ({})", text, u.target)
        }
        InlineMacro::Autolink(a) => a.url.to_string(),
        InlineMacro::Image(i) => format!("{} ({})", i.source, inlines_to_string(i.title.as_ref())),
        InlineMacro::Footnote(f) => {
            let id = f.id.unwrap_or("");
            format!("[{}]", id)
        }
        InlineMacro::Mailto(mt) => {
            let text = if mt.text.is_empty() {
                mt.target.to_string()
            } else {
                inlines_to_string(&mt.text).into_string()
            };
            format!("{} ({})", text, mt.target)
        }
        InlineMacro::CrossReference(xr) => {
            let text = if xr.text.is_empty() {
                xr.target.to_string()
            } else {
                inlines_to_string(&xr.text).into_string()
            };
            format!("{} <<{}>>", text, xr.target)
        }
        _ => "[macro]".to_string(),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
