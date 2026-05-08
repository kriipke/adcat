// Copyright 2025
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Convert AsciiDoc AST to pulldown-cmark events.

use acdc_parser::{
    AttributeValue, Block, CalloutList, DelimitedBlockType, DescriptionList, Document, Footnote,
    InlineMacro, InlineNode, ListItem, OrderedList, Paragraph, Section, TocEntry, UnorderedList,
};
use pulldown_cmark::{Alignment, CowStr, Event, HeadingLevel, LinkType, Tag, TagEnd};

pub(crate) const TABLE_FOOTER_MARKER: &str = "<!--xcat:table-footer-->";

struct RenderContext<'a> {
    toc_entries: &'a [TocEntry<'a>],
    next_toc_entry: usize,
}

/// Convert an AsciiDoc document to pulldown-cmark events.
pub fn document_to_events(doc: &Document) -> Vec<Event<'static>> {
    let mut context = RenderContext {
        toc_entries: &doc.toc_entries,
        next_toc_entry: 0,
    };
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
    events.extend(document_metadata_to_events(doc));

    for block in &doc.blocks {
        events.extend(block_to_events(block, &mut context));
    }

    for footnote in &doc.footnotes {
        events.extend(footnote_definition_to_events(footnote));
    }

    events
}

fn heading_level(level: impl Into<usize>) -> HeadingLevel {
    match level.into().saturating_add(1) {
        1 => HeadingLevel::H1,
        2 => HeadingLevel::H2,
        3 => HeadingLevel::H3,
        4 => HeadingLevel::H4,
        5 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    }
}

fn footnote_label(footnote: &Footnote) -> CowStr<'static> {
    if footnote.number != 0 {
        footnote.number.to_string().into()
    } else if let Some(id) = footnote.id {
        id.to_string().into()
    } else {
        CowStr::from("")
    }
}

fn unsupported_block_events(name: &str) -> Vec<Event<'static>> {
    vec![
        Event::Start(Tag::Paragraph),
        Event::Text(format!("[unsupported AsciiDoc {name} block]").into()),
        Event::End(TagEnd::Paragraph),
    ]
}

fn footnote_definition_to_events(footnote: &Footnote) -> Vec<Event<'static>> {
    let label = footnote_label(footnote);
    let mut events = vec![Event::Start(Tag::FootnoteDefinition(label.clone()))];
    if !footnote.content.is_empty() {
        events.push(Event::Start(Tag::Paragraph));
        events.extend(inlines_to_events(&footnote.content));
        events.push(Event::End(TagEnd::Paragraph));
    }
    events.push(Event::End(TagEnd::FootnoteDefinition));
    events
}

fn link_with_inline_text_events(
    link_type: LinkType,
    dest_url: CowStr<'static>,
    text: &[InlineNode],
) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::Link {
        link_type,
        dest_url: dest_url.clone(),
        title: CowStr::from(""),
        id: CowStr::from(""),
    })];
    if text.is_empty() {
        events.push(Event::Text(dest_url));
    } else {
        events.extend(inlines_to_events(text));
    }
    events.push(Event::End(TagEnd::Link));
    events
}

fn title_to_events(title: &acdc_parser::Title, level: HeadingLevel) -> Vec<Event<'static>> {
    title_to_events_with_id(title, level, None)
}

fn title_to_events_with_id(
    title: &acdc_parser::Title,
    level: HeadingLevel,
    id: Option<CowStr<'static>>,
) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    events.push(Event::Start(Tag::Heading {
        level,
        id,
        classes: vec![],
        attrs: vec![],
    }));
    events.extend(inlines_to_events(title.as_ref()));
    events.push(Event::End(TagEnd::Heading(level)));
    events
}

fn section_to_events(section: &Section, context: &mut RenderContext<'_>) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    let level = heading_level(section.level);
    let section_id = context.next_section_id().map(|id| CowStr::from(id.to_string()));
    events.extend(title_to_events_with_id(&section.title, level, section_id));

    for block in &section.content {
        events.extend(block_to_events(block, context));
    }

    events
}

fn block_to_events(block: &Block, context: &mut RenderContext<'_>) -> Vec<Event<'static>> {
    match block {
        Block::Paragraph(para) => paragraph_to_events(para),
        Block::Section(section) => section_to_events(section, context),
        Block::UnorderedList(list) => unordered_list_to_events(list, context),
        Block::OrderedList(list) => ordered_list_to_events(list, context),
        Block::CalloutList(list) => callout_list_to_events(list, context),
        Block::DescriptionList(list) => description_list_to_events(list, context),
        Block::DelimitedBlock(delimited) => delimited_block_to_events(delimited, context),
        Block::ThematicBreak(_) => vec![Event::Rule],
        Block::PageBreak(_) => vec![Event::Rule],
        Block::DiscreteHeader(header) => {
            let level = heading_level(header.level);
            title_to_events(&header.title, level)
        }
        Block::Image(img) => {
            let dest = source_to_cowstr(&img.source);
            let mut events = vec![
                Event::Start(Tag::Paragraph),
                Event::Start(Tag::Image {
                    link_type: LinkType::Inline,
                    dest_url: dest,
                    title: CowStr::from(""),
                    id: CowStr::from(""),
                }),
            ];
            events.extend(inlines_to_events(img.title.as_ref()));
            events.push(Event::End(TagEnd::Image));
            events.push(Event::End(TagEnd::Paragraph));
            events
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
                events.extend(block_to_events(block, context));
            }
            events
        }
        Block::Comment(_) => vec![],
        Block::DocumentAttribute(_) => vec![],
        Block::TableOfContents(_) => table_of_contents_to_events(context.toc_entries),
        Block::Audio(audio) => {
            media_block_to_events("Audio", &audio.title, &[audio.source.to_string()])
        }
        Block::Video(video) => video_block_to_events(video),
        _ => unsupported_block_events("block"),
    }
}

impl<'a> RenderContext<'a> {
    fn next_section_id(&mut self) -> Option<&'a str> {
        let entry = self.toc_entries.get(self.next_toc_entry)?;
        self.next_toc_entry += 1;
        Some(entry.id)
    }
}

fn document_metadata_to_events(doc: &Document) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    if let Some(description) = doc.attributes.get("description").and_then(attribute_to_text) {
        events.push(Event::Start(Tag::Paragraph));
        events.push(Event::Text(description.into()));
        events.push(Event::End(TagEnd::Paragraph));
    }

    let revision = [
        doc.attributes.get("revnumber").and_then(attribute_to_text),
        doc.attributes.get("revdate").and_then(attribute_to_text),
        doc.attributes.get("revremark").and_then(attribute_to_text),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !revision.is_empty() {
        events.push(Event::Start(Tag::Paragraph));
        events.push(Event::Text(revision.join(", ").into()));
        events.push(Event::End(TagEnd::Paragraph));
    }

    events
}

fn attribute_to_text(value: &AttributeValue<'_>) -> Option<String> {
    match value {
        AttributeValue::String(value) if !value.is_empty() => Some(value.to_string()),
        AttributeValue::Bool(true) => Some("true".to_string()),
        _ => None,
    }
}

fn table_of_contents_to_events(toc_entries: &[TocEntry<'_>]) -> Vec<Event<'static>> {
    if toc_entries.is_empty() {
        return vec![];
    }

    let mut events = vec![Event::Start(Tag::Paragraph)];
    events.push(Event::Text("Table of Contents".into()));
    events.push(Event::End(TagEnd::Paragraph));
    events.push(Event::Start(Tag::List(None)));
    for entry in toc_entries {
        events.push(Event::Start(Tag::Item));
        if entry.level > 1 {
            events.push(Event::Text("  ".repeat(entry.level.saturating_sub(1) as usize).into()));
        }
        events.push(Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url: format!("#{}", entry.id).into(),
            title: CowStr::from(""),
            id: CowStr::from(""),
        }));
        events.extend(inlines_to_events(entry.title.as_ref()));
        events.push(Event::End(TagEnd::Link));
        events.push(Event::End(TagEnd::Item));
    }
    events.push(Event::End(TagEnd::List(false)));
    events
}

fn media_block_to_events(
    label: &str,
    title: &acdc_parser::Title,
    sources: &[String],
) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::Paragraph)];
    events.push(Event::Text(label.to_string().into()));
    if !title.is_empty() {
        events.push(Event::Text(": ".into()));
        events.extend(inlines_to_events(title.as_ref()));
    }
    events.push(Event::End(TagEnd::Paragraph));
    events.push(Event::Start(Tag::List(None)));
    for source in sources {
        events.push(Event::Start(Tag::Item));
        events.push(Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url: source.clone().into(),
            title: CowStr::from(""),
            id: CowStr::from(""),
        }));
        events.push(Event::Text(source.clone().into()));
        events.push(Event::End(TagEnd::Link));
        events.push(Event::End(TagEnd::Item));
    }
    events.push(Event::End(TagEnd::List(false)));
    events
}

fn video_block_to_events(video: &acdc_parser::Video) -> Vec<Event<'static>> {
    let sources = video.sources.iter().map(ToString::to_string).collect::<Vec<_>>();
    media_block_to_events("Video", &video.title, &sources)
}

fn table_alignment(
    table: &acdc_parser::Table,
    column_index: usize,
    cell: &acdc_parser::TableColumn,
) -> Alignment {
    let halign = cell
        .halign
        .or_else(|| table.columns.get(column_index).map(|column| column.halign));
    match halign.unwrap_or(acdc_parser::HorizontalAlignment::Left) {
        acdc_parser::HorizontalAlignment::Left => Alignment::Left,
        acdc_parser::HorizontalAlignment::Center => Alignment::Center,
        acdc_parser::HorizontalAlignment::Right => Alignment::Right,
    }
}

#[derive(Clone, Debug)]
struct ExpandedTableCell {
    text: String,
    alignment: Alignment,
}

#[derive(Clone, Debug)]
struct PendingRowSpan {
    remaining_rows: usize,
    alignment: Alignment,
}

fn consume_pending_rowspans(
    pending_rowspans: &mut [Option<PendingRowSpan>],
    expanded: &mut Vec<ExpandedTableCell>,
    column_index: &mut usize,
) {
    while pending_rowspans
        .get(*column_index)
        .and_then(|span| span.as_ref())
        .is_some()
    {
        let span = pending_rowspans[*column_index]
            .as_mut()
            .expect("checked is_some");
        expanded.push(ExpandedTableCell {
            text: String::new(),
            alignment: span.alignment,
        });
        span.remaining_rows = span.remaining_rows.saturating_sub(1);
        if span.remaining_rows == 0 {
            pending_rowspans[*column_index] = None;
        }
        *column_index += 1;
    }
}

fn push_table_part(parts: &mut Vec<String>, text: String) {
    if !text.is_empty() {
        parts.push(text);
    }
}

fn table_prefixed_lines(prefix: &str, text: &str) -> String {
    text.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn admonition_label(variant: &acdc_parser::AdmonitionVariant) -> &'static str {
    match variant {
        acdc_parser::AdmonitionVariant::Note => "NOTE",
        acdc_parser::AdmonitionVariant::Tip => "TIP",
        acdc_parser::AdmonitionVariant::Important => "IMPORTANT",
        acdc_parser::AdmonitionVariant::Caution => "CAUTION",
        acdc_parser::AdmonitionVariant::Warning => "WARNING",
    }
}

fn expand_table_row(
    row: &acdc_parser::TableRow,
    table: &acdc_parser::Table,
    pending_rowspans: &mut Vec<Option<PendingRowSpan>>,
) -> Vec<ExpandedTableCell> {
    let mut expanded = Vec::new();
    let mut column_index = 0usize;

    for cell in &row.columns {
        consume_pending_rowspans(pending_rowspans.as_mut_slice(), &mut expanded, &mut column_index);

        let alignment = table_alignment(table, column_index, cell);
        let colspan = cell.colspan.max(1);
        let rowspan = cell.rowspan.max(1);

        expanded.push(ExpandedTableCell {
            text: table_cell_text(&cell.content),
            alignment,
        });
        if pending_rowspans.len() <= column_index {
            pending_rowspans.resize(column_index + 1, None);
        }
        if rowspan > 1 {
            pending_rowspans[column_index] = Some(PendingRowSpan {
                remaining_rows: rowspan - 1,
                alignment,
            });
        }
        column_index += 1;

        for _ in 1..colspan {
            expanded.push(ExpandedTableCell {
                text: String::new(),
                alignment,
            });
            if pending_rowspans.len() <= column_index {
                pending_rowspans.resize(column_index + 1, None);
            }
            if rowspan > 1 {
                pending_rowspans[column_index] = Some(PendingRowSpan {
                    remaining_rows: rowspan - 1,
                    alignment,
                });
            }
            column_index += 1;
        }
    }

    consume_pending_rowspans(pending_rowspans.as_mut_slice(), &mut expanded, &mut column_index);
    expanded
}

fn table_cell_text(blocks: &[Block]) -> String {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                push_table_part(&mut parts, inlines_to_string(&paragraph.content).into_string());
            }
            Block::Image(image) => {
                push_table_part(&mut parts, inlines_to_string(image.title.as_ref()).into_string());
            }
            Block::UnorderedList(list) => {
                for item in &list.items {
                    let principal = inlines_to_string(&item.principal).into_string();
                    if !principal.is_empty() {
                        push_table_part(&mut parts, format!("* {principal}"));
                    }
                    let nested = table_cell_text(&item.blocks);
                    if !nested.is_empty() {
                        push_table_part(&mut parts, table_prefixed_lines("  ", &nested));
                    }
                }
            }
            Block::OrderedList(list) => {
                let start = list
                    .marker
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<usize>()
                    .ok()
                    .filter(|start| *start > 0)
                    .unwrap_or(1);
                for (offset, item) in list.items.iter().enumerate() {
                    let principal = inlines_to_string(&item.principal).into_string();
                    if !principal.is_empty() {
                        push_table_part(&mut parts, format!("{}. {principal}", start + offset));
                    }
                    let nested = table_cell_text(&item.blocks);
                    if !nested.is_empty() {
                        push_table_part(&mut parts, table_prefixed_lines("  ", &nested));
                    }
                }
            }
            Block::DescriptionList(list) => {
                for item in &list.items {
                    let mut part = inlines_to_string(&item.term).into_string();
                    if !item.principal_text.is_empty() {
                        if !part.is_empty() {
                            part.push_str(": ");
                        }
                        part.push_str(&inlines_to_string(&item.principal_text));
                    }
                    push_table_part(&mut parts, part);
                    let nested = table_cell_text(&item.description);
                    if !nested.is_empty() {
                        push_table_part(&mut parts, table_prefixed_lines("  ", &nested));
                    }
                }
            }
            Block::CalloutList(list) => {
                for item in &list.items {
                    let principal = inlines_to_string(&item.principal).into_string();
                    if !principal.is_empty() {
                        push_table_part(&mut parts, format!("<{}> {principal}", item.callout.number));
                    }
                    let nested = table_cell_text(&item.blocks);
                    if !nested.is_empty() {
                        push_table_part(&mut parts, table_prefixed_lines("  ", &nested));
                    }
                }
            }
            Block::DelimitedBlock(delimited) => match &delimited.inner {
                DelimitedBlockType::DelimitedListing(inlines)
                | DelimitedBlockType::DelimitedLiteral(inlines)
                | DelimitedBlockType::DelimitedPass(inlines)
                | DelimitedBlockType::DelimitedVerse(inlines) => {
                    push_table_part(&mut parts, inlines_to_string(inlines).into_string());
                }
                DelimitedBlockType::DelimitedStem(stem) => {
                    push_table_part(&mut parts, stem.content.to_string());
                }
                DelimitedBlockType::DelimitedTable(inner_table) => {
                    let mut nested_rows = Vec::new();
                    if let Some(header) = &inner_table.header {
                        nested_rows.push(
                            header
                                .columns
                                .iter()
                                .map(|column| table_cell_text(&column.content))
                                .collect::<Vec<_>>()
                                .join(" | "),
                        );
                    }
                    for row in &inner_table.rows {
                        nested_rows.push(
                            row.columns
                                .iter()
                                .map(|column| table_cell_text(&column.content))
                                .collect::<Vec<_>>()
                                .join(" | "),
                        );
                    }
                    push_table_part(&mut parts, nested_rows.join("\n"));
                }
                DelimitedBlockType::DelimitedExample(blocks)
                | DelimitedBlockType::DelimitedOpen(blocks)
                | DelimitedBlockType::DelimitedSidebar(blocks)
                | DelimitedBlockType::DelimitedQuote(blocks) => {
                    let nested = table_cell_text(blocks);
                    push_table_part(&mut parts, nested);
                }
                DelimitedBlockType::DelimitedComment(_) => {}
                _ => {}
            },
            Block::Admonition(admonition) => {
                let nested = table_cell_text(&admonition.blocks);
                if nested.is_empty() {
                    push_table_part(
                        &mut parts,
                        format!("{}:", admonition_label(&admonition.variant)),
                    );
                } else {
                    push_table_part(
                        &mut parts,
                        format!(
                            "{}: {}",
                            admonition_label(&admonition.variant),
                            nested.replace('\n', " ")
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    parts.join("\n")
}

fn table_row_to_events(row: &[ExpandedTableCell]) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::TableRow)];
    for column in row {
        events.push(Event::Start(Tag::TableCell));
        events.push(Event::Text(column.text.clone().into()));
        events.push(Event::End(TagEnd::TableCell));
    }
    events.push(Event::End(TagEnd::TableRow));
    events
}

fn table_to_events(table: &acdc_parser::Table) -> Vec<Event<'static>> {
    let mut pending_rowspans = Vec::new();
    let expanded_header = table
        .header
        .as_ref()
        .map(|row| expand_table_row(row, table, &mut pending_rowspans));
    let expanded_rows = table
        .rows
        .iter()
        .map(|row| expand_table_row(row, table, &mut pending_rowspans))
        .collect::<Vec<_>>();
    let expanded_footer = table
        .footer
        .as_ref()
        .map(|row| expand_table_row(row, table, &mut pending_rowspans));

    let alignments = expanded_header
        .as_ref()
        .or_else(|| expanded_rows.first())
        .or(expanded_footer.as_ref())
        .map(|row| row.iter().map(|cell| cell.alignment).collect())
        .unwrap_or_default();

    let mut events = vec![Event::Start(Tag::Table(alignments))];
    if let Some(header) = &expanded_header {
        events.push(Event::Start(Tag::TableHead));
        events.extend(table_row_to_events(header));
        events.push(Event::End(TagEnd::TableHead));
    }
    for row in &expanded_rows {
        events.extend(table_row_to_events(row));
    }
    if let Some(footer) = &expanded_footer {
        events.push(Event::InlineHtml(TABLE_FOOTER_MARKER.into()));
        events.extend(table_row_to_events(footer));
    }
    events.push(Event::End(TagEnd::Table));
    events
}

fn paragraph_to_events(para: &Paragraph) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::Paragraph)];
    events.extend(inlines_to_events(&para.content));
    events.push(Event::End(TagEnd::Paragraph));
    events
}

fn list_item_to_events(item: &ListItem, context: &mut RenderContext<'_>) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::Item)];
    events.extend(inlines_to_events(&item.principal));
    for block in &item.blocks {
        events.extend(block_to_events(block, context));
    }
    events.push(Event::End(TagEnd::Item));
    events
}

fn unordered_list_to_events(
    list: &UnorderedList,
    context: &mut RenderContext<'_>,
) -> Vec<Event<'static>> {
    let mut events = vec![Event::Start(Tag::List(None))];
    for item in &list.items {
        events.extend(list_item_to_events(item, context));
    }
    events.push(Event::End(TagEnd::List(false)));
    events
}

fn ordered_list_to_events(
    list: &OrderedList,
    context: &mut RenderContext<'_>,
) -> Vec<Event<'static>> {
    if let Some(markers) = non_numeric_ordered_list_markers(list) {
        let mut events = Vec::new();
        for (item, marker) in list.items.iter().zip(markers) {
            events.push(Event::Start(Tag::Paragraph));
            events.push(Event::Text(format!("{marker} ").into()));
            events.extend(inlines_to_events(&item.principal));
            events.push(Event::End(TagEnd::Paragraph));
            for block in &item.blocks {
                events.extend(block_to_events(block, context));
            }
        }
        return events;
    }

    let start = list
        .marker
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .ok()
        .filter(|start| *start > 0)
        .unwrap_or(1);
    let mut events = vec![Event::Start(Tag::List(Some(start)))];
    for item in &list.items {
        events.extend(list_item_to_events(item, context));
    }
    events.push(Event::End(TagEnd::List(true)));
    events
}

fn non_numeric_ordered_list_markers(list: &OrderedList) -> Option<Vec<String>> {
    match list.metadata.style {
        Some("loweralpha") => {
            return Some(
                (1..=list.items.len())
                    .map(|value| format!("{}.", alpha_marker(value, false)))
                    .collect(),
            );
        }
        Some("upperalpha") => {
            return Some(
                (1..=list.items.len())
                    .map(|value| format!("{}.", alpha_marker(value, true)))
                    .collect(),
            );
        }
        Some("lowerroman") => {
            return Some(
                (1..=list.items.len())
                    .map(|value| format!("{}.", number_to_roman(value, false)))
                    .collect(),
            );
        }
        Some("upperroman") => {
            return Some(
                (1..=list.items.len())
                    .map(|value| format!("{}.", number_to_roman(value, true)))
                    .collect(),
            );
        }
        _ => {}
    }

    let marker = list.marker.trim();
    let trimmed = marker.trim_end_matches(['.', ')']);

    if trimmed.len() == 1 && trimmed.chars().all(|c| c.is_ascii_lowercase()) {
        let start = (trimmed.as_bytes()[0] - b'a' + 1) as usize;
        return Some(
            (start..start + list.items.len())
                .map(|value| format!("{}.", alpha_marker(value, false)))
                .collect(),
        );
    }
    if trimmed.len() == 1 && trimmed.chars().all(|c| c.is_ascii_uppercase()) {
        let start = (trimmed.as_bytes()[0] - b'A' + 1) as usize;
        return Some(
            (start..start + list.items.len())
                .map(|value| format!("{}.", alpha_marker(value, true)))
                .collect(),
        );
    }
    if trimmed.chars().all(|c| matches!(c, 'i' | 'v' | 'x' | 'l' | 'c' | 'd' | 'm')) {
        let start = roman_to_number(trimmed)?;
        return Some(
            (start..start + list.items.len())
                .map(|value| format!("{}.", number_to_roman(value, false)))
                .collect(),
        );
    }
    if trimmed
        .chars()
        .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
    {
        let start = roman_to_number(trimmed)?;
        return Some(
            (start..start + list.items.len())
                .map(|value| format!("{}.", number_to_roman(value, true)))
                .collect(),
        );
    }

    None
}

fn alpha_marker(mut value: usize, uppercase: bool) -> String {
    let mut chars = Vec::new();
    while value > 0 {
        value -= 1;
        let ch = (b'a' + (value % 26) as u8) as char;
        chars.push(if uppercase { ch.to_ascii_uppercase() } else { ch });
        value /= 26;
    }
    chars.iter().rev().collect()
}

fn roman_to_number(roman: &str) -> Option<usize> {
    let mut total = 0usize;
    let mut prev = 0usize;
    for ch in roman.chars().rev() {
        let value = match ch.to_ascii_uppercase() {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };
        if value < prev {
            total = total.checked_sub(value)?;
        } else {
            total += value;
            prev = value;
        }
    }
    Some(total)
}

fn number_to_roman(mut value: usize, uppercase: bool) -> String {
    let mut output = String::new();
    const NUMERALS: &[(usize, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    for (number, numeral) in NUMERALS {
        while value >= *number {
            value -= *number;
            output.push_str(numeral);
        }
    }
    if uppercase {
        output
    } else {
        output.to_ascii_lowercase()
    }
}

fn callout_list_to_events(
    list: &CalloutList,
    context: &mut RenderContext<'_>,
) -> Vec<Event<'static>> {
    let start = list.items.first().map(|item| item.callout.number as u64);
    let mut events = vec![Event::Start(Tag::List(start))];
    for item in &list.items {
        let mut item_events = vec![Event::Start(Tag::Item)];
        item_events.extend(inlines_to_events(&item.principal));
        for block in &item.blocks {
            item_events.extend(block_to_events(block, context));
        }
        item_events.push(Event::End(TagEnd::Item));
        events.extend(item_events);
    }
    events.push(Event::End(TagEnd::List(true)));
    events
}

fn description_list_to_events(
    list: &DescriptionList,
    context: &mut RenderContext<'_>,
) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    for item in &list.items {
        events.push(Event::Start(Tag::Paragraph));
        events.extend(inlines_to_events(&item.term));
        events.push(Event::Text(": ".into()));
        events.extend(inlines_to_events(&item.principal_text));
        events.push(Event::End(TagEnd::Paragraph));
        for block in &item.description {
            events.extend(block_to_events(block, context));
        }
    }
    events
}

fn delimited_block_to_events(
    block: &acdc_parser::DelimitedBlock,
    context: &mut RenderContext<'_>,
) -> Vec<Event<'static>> {
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
                events.extend(block_to_events(block, context));
            }
            events.push(Event::End(TagEnd::BlockQuote(None)));
            events
        }
        DelimitedBlockType::DelimitedExample(blocks)
        | DelimitedBlockType::DelimitedSidebar(blocks)
        | DelimitedBlockType::DelimitedOpen(blocks) => {
            let mut events = Vec::new();
            for block in blocks {
                events.extend(block_to_events(block, context));
            }
            events
        }
        DelimitedBlockType::DelimitedPass(inlines) => {
            let mut events = vec![Event::Start(Tag::Paragraph)];
            events.extend(inlines_to_events(inlines));
            events.push(Event::End(TagEnd::Paragraph));
            events
        }
        DelimitedBlockType::DelimitedVerse(inlines) => {
            let mut events = vec![Event::Start(Tag::BlockQuote(None)), Event::Start(Tag::Paragraph)];
            events.extend(inlines_to_events(inlines));
            events.push(Event::End(TagEnd::Paragraph));
            events.push(Event::End(TagEnd::BlockQuote(None)));
            events
        }
        DelimitedBlockType::DelimitedStem(stem) => {
            let mut text = stem.content.to_string();
            if !text.ends_with('\n') {
                text.push('\n');
            }
            vec![
                Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(
                    stem.notation.to_string().into(),
                ))),
                Event::Text(text.into()),
                Event::End(TagEnd::CodeBlock),
            ]
        }
        DelimitedBlockType::DelimitedTable(table) => table_to_events(table),
        DelimitedBlockType::DelimitedComment(_) | _ => vec![],
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
        InlineNode::CurvedApostropheText(quote) => {
            let mut events = vec![Event::Text(CowStr::from("\u{2018}"))];
            events.extend(inlines_to_events(&quote.content));
            events.push(Event::Text(CowStr::from("\u{2019}")));
            events
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
            link_with_inline_text_events(LinkType::Inline, dest, &link.text)
        }
        InlineMacro::Url(url) => {
            let dest = source_to_cowstr(&url.target);
            link_with_inline_text_events(LinkType::Inline, dest, &url.text)
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
            let mut events = vec![Event::Start(Tag::Image {
                link_type: LinkType::Inline,
                dest_url: dest,
                title: CowStr::from(""),
                id: CowStr::from(""),
            })];
            events.extend(inlines_to_events(img.title.as_ref()));
            events.push(Event::End(TagEnd::Image));
            events
        }
        InlineMacro::Footnote(footnote) => {
            vec![Event::FootnoteReference(footnote_label(footnote))]
        }
        InlineMacro::Mailto(mailto) => {
            let dest = source_to_cowstr(&mailto.target);
            link_with_inline_text_events(LinkType::Email, dest, &mailto.text)
        }
        InlineMacro::CrossReference(xref) => {
            link_with_inline_text_events(
                LinkType::Inline,
                format!("#{}", xref.target).into(),
                xref.text.as_ref(),
            )
        }
        InlineMacro::Pass(pass) => vec![Event::Code(pass.text.unwrap_or("").to_string().into())],
        InlineMacro::Button(button) => vec![Event::Code(button.label.to_string().into())],
        InlineMacro::Keyboard(keyboard) => {
            vec![Event::Code(keyboard.keys.join("+").into())]
        }
        InlineMacro::Menu(menu) => {
            let mut path = String::from(menu.target);
            if !menu.items.is_empty() {
                path.push_str(" > ");
                path.push_str(&menu.items.join(" > "));
            }
            vec![Event::Text(path.into())]
        }
        InlineMacro::Stem(stem) => {
            vec![Event::Code(format!("{}: {}", stem.notation, stem.content).into())]
        }
        InlineMacro::Icon(icon) => vec![Event::Text(icon.target.to_string().into())],
        InlineMacro::IndexTerm(index_term) => {
            if matches!(
                &index_term.kind,
                acdc_parser::IndexTermKind::Flow(_)
            ) {
                vec![Event::Text(index_term.term().to_string().into())]
            } else {
                vec![]
            }
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
            InlineNode::HighlightText(h) => {
                result.push_str(&inlines_to_string(&h.content));
            }
            InlineNode::SubscriptText(s) => {
                result.push_str(&inlines_to_string(&s.content));
            }
            InlineNode::SuperscriptText(s) => {
                result.push_str(&inlines_to_string(&s.content));
            }
            InlineNode::CurvedQuotationText(q) => {
                result.push('\u{201c}');
                result.push_str(&inlines_to_string(&q.content));
                result.push('\u{201d}');
            }
            InlineNode::CurvedApostropheText(a) => {
                result.push('\u{2018}');
                result.push_str(&inlines_to_string(&a.content));
                result.push('\u{2019}');
            }
            InlineNode::StandaloneCurvedApostrophe(_) => result.push('\u{2019}'),
            InlineNode::LineBreak(_) => result.push('\n'),
            InlineNode::InlineAnchor(_) => {}
            InlineNode::CalloutRef(callout) => result.push_str(&format!("<{}>", callout.number)),
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
            if f.number != 0 {
                format!("[{}]", f.number)
            } else {
                let id = f.id.unwrap_or("");
                format!("[{}]", id)
            }
        }
        InlineMacro::Pass(pass) => pass.text.unwrap_or("").to_string(),
        InlineMacro::Button(button) => button.label.to_string(),
        InlineMacro::Keyboard(keyboard) => keyboard.keys.join("+"),
        InlineMacro::Menu(menu) => {
            let mut path = String::from(menu.target);
            if !menu.items.is_empty() {
                path.push_str(" > ");
                path.push_str(&menu.items.join(" > "));
            }
            path
        }
        InlineMacro::Stem(stem) => stem.content.to_string(),
        InlineMacro::Icon(icon) => icon.target.to_string(),
        InlineMacro::IndexTerm(index_term) => {
            if matches!(&index_term.kind, acdc_parser::IndexTermKind::Flow(_)) {
                index_term.term().to_string()
            } else {
                String::new()
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_events(input: &str) -> Vec<Event<'static>> {
        let parsed = acdc_parser::parse(input, &acdc_parser::Options::default()).unwrap();
        document_to_events(parsed.document())
    }

    #[test]
    fn section_levels_follow_asciidoc_hierarchy() {
        let events = parse_events("= Title\n\n== Section\n\n=== Subsection\n");

        let headings: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Start(Tag::Heading { level, .. }) => Some(*level),
                _ => None,
            })
            .collect();

        assert_eq!(headings, vec![HeadingLevel::H1, HeadingLevel::H2, HeadingLevel::H3]);
    }

    #[test]
    fn paragraphs_preserve_inline_formatting() {
        let events = parse_events("A *bold* _italic_ https://example.com[] word.\n");

        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::Strong))));
        assert!(events.iter().any(|event| matches!(event, Event::End(TagEnd::Strong))));
        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::Emphasis))));
        assert!(events.iter().any(|event| matches!(event, Event::End(TagEnd::Emphasis))));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Start(Tag::Link {
                dest_url,
                ..
            }) if dest_url.as_ref() == "https://example.com"
        )));
    }

    #[test]
    fn list_items_preserve_inline_formatting() {
        let events = parse_events("* item with *bold*\n");

        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::Item))));
        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::Strong))));
    }

    #[test]
    fn container_blocks_do_not_wrap_blocks_in_paragraphs() {
        let events = parse_events("====\nA paragraph\n\n* list item\n====\n");

        let paragraph_end = events
            .iter()
            .position(|event| matches!(event, Event::End(TagEnd::Paragraph)))
            .expect("expected paragraph end");
        let list_start = events
            .iter()
            .position(|event| matches!(event, Event::Start(Tag::List(None))))
            .expect("expected list start");

        assert!(paragraph_end < list_start);
    }

    #[test]
    fn ordered_lists_preserve_numeric_start() {
        let events = parse_events("4. fourth\n5. fifth\n");

        assert!(matches!(
            events.first(),
            Some(Event::Start(Tag::List(Some(4))))
        ));
    }

    #[test]
    fn alphabetic_ordered_lists_preserve_visible_markers() {
        let events = parse_events("[loweralpha]\n. first\n. second\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "a. "
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "b. "
        )));
    }

    #[test]
    fn roman_ordered_lists_preserve_visible_markers() {
        let events = parse_events("[upperroman]\n. first\n. second\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "I. "
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "II. "
        )));
    }

    #[test]
    fn footnotes_emit_references_and_definitions() {
        let events = parse_events("Footnote footnote:[hello world].\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::FootnoteReference(label) if label.as_ref() == "1"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Start(Tag::FootnoteDefinition(label)) if label.as_ref() == "1"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "hello world"
        )));
    }

    #[test]
    fn verse_blocks_are_no_longer_dropped() {
        let events = parse_events("[verse]\n____\nRoses are red\nViolets are blue\n____\n");

        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::BlockQuote(None)))));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref().contains("Roses are red")
        )));
    }

    #[test]
    fn formatted_link_text_is_preserved_inside_link_events() {
        let events = parse_events("link:https://example.com[*bold* text]\n");

        let link_start = events
            .iter()
            .position(|event| matches!(event, Event::Start(Tag::Link { .. })))
            .expect("expected link start");
        let strong_start = events
            .iter()
            .position(|event| matches!(event, Event::Start(Tag::Strong)))
            .expect("expected strong start");
        let link_end = events
            .iter()
            .position(|event| matches!(event, Event::End(TagEnd::Link)))
            .expect("expected link end");

        assert!(link_start < strong_start);
        assert!(strong_start < link_end);
    }

    #[test]
    fn curved_apostrophe_text_preserves_inner_content() {
        let events = parse_events("'`quoted`'\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "\u{2018}"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "quoted"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "\u{2019}"
        )));
    }

    #[test]
    fn description_lists_are_no_longer_unsupported_blocks() {
        let events = parse_events("term:: explanation\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "term"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == ": "
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "explanation"
        )));
        assert!(!events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref().contains("[unsupported")
        )));
    }

    #[test]
    fn callout_lists_are_rendered_as_lists() {
        let events = parse_events(
            "[source]\n----\nlet x = 1; <1>\n----\n<1> first callout\n",
        );

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Start(Tag::List(Some(1)))
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "first callout"
        )));
    }

    #[test]
    fn simple_tables_are_rendered_as_table_events() {
        let events = parse_events("|===\n| A | B\n| C | D\n|===\n");

        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::Table(_)))));
        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::TableRow))));
        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::TableCell))));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "A"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "D"
        )));
    }

    #[test]
    fn header_tables_emit_table_head() {
        let events = parse_events("[options=\"header\"]\n|===\n| Name | Age\n| Ada | 42\n|===\n");

        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::TableHead))));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "Name"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "42"
        )));
    }

    #[test]
    fn table_footers_emit_footer_marker_before_footer_row() {
        let events = parse_events(
            "[options=\"footer\"]\n|===\n| Name | Value\n| body | 1\n| footer | 2\n|===\n",
        );

        let footer_marker_index = events
            .iter()
            .position(
                |event| matches!(event, Event::InlineHtml(html) if html.as_ref() == TABLE_FOOTER_MARKER),
            )
            .expect("expected footer marker");
        let footer_row_index = events
            .iter()
            .enumerate()
            .skip(footer_marker_index + 1)
            .find_map(|(index, event)| matches!(event, Event::Start(Tag::TableRow)).then_some(index))
            .expect("expected footer row");

        assert!(footer_marker_index < footer_row_index);
    }

    #[test]
    fn toc_blocks_render_links_for_sections() {
        let events = parse_events("= Doc\n\ntoc::[]\n\n== Section A\n\n=== Section B\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "Table of Contents"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Start(Tag::Link { dest_url, .. }) if dest_url.as_ref() == "#_section_a"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Start(Tag::Link { dest_url, .. }) if dest_url.as_ref() == "#_section_b"
        )));
    }

    #[test]
    fn section_headings_use_toc_entry_ids() {
        let events = parse_events("= Doc\n\n== Section A\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Start(Tag::Heading { id: Some(id), .. }) if id.as_ref() == "_section_a"
        )));
    }

    #[test]
    fn document_description_attribute_is_rendered() {
        let events = parse_events("= Doc\n:description: Summary line\n\nBody.\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "Summary line"
        )));
    }

    #[test]
    fn video_blocks_render_as_titled_link_lists() {
        let events = parse_events(".Launch Demo\nvideo::movie.mp4[]\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "Video"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "Launch Demo"
        )));
        assert!(events.iter().any(|event| matches!(event, Event::Start(Tag::List(None)))));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Start(Tag::Link { dest_url, .. }) if dest_url.as_ref() == "movie.mp4"
        )));
    }

    #[test]
    fn stem_macros_render_as_code_with_notation() {
        let events = parse_events("stem:[x^2]\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Code(text) if text.as_ref().contains("x^2")
        )));
    }

    #[test]
    fn asciidoc_style_table_cells_preserve_list_structure_in_text() {
        let events = parse_events("[cols=\"1a\"]\n|===\na|\n* one\n* two\n|===\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref().contains("* one")
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref().contains("* two")
        )));
    }

    #[test]
    fn table_cells_preserve_admonition_labels() {
        let events = parse_events("[cols=\"1a\"]\n|===\na|\nNOTE: inside\n|===\n");

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref().contains("NOTE:")
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref().contains("inside")
        )));
    }

    #[test]
    fn table_spans_expand_to_rectangular_grid() {
        let events = parse_events("|===\n2+| wide | tail\n| next\n|===\n");

        let cells = events
            .iter()
            .filter(|event| matches!(event, Event::Start(Tag::TableCell)))
            .count();
        assert_eq!(cells, 4);
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "wide"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "tail"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Text(text) if text.as_ref() == "next"
        )));
    }
}
