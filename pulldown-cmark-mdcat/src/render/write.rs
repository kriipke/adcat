// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::cmp::{max, min};
use std::io::{Result, Write};
use std::iter::zip;

use anstyle::Style;
use pulldown_cmark::{Alignment, CodeBlockKind, HeadingLevel};
use syntect::highlighting::HighlightState;
use syntect::parsing::{ParseState, ScopeStack};
use textwrap::core::{display_width, Word};
use textwrap::WordSeparator;

use crate::references::*;
use crate::render::data::{CurrentLine, CurrentTable, LinkReferenceDefinition, TableCell};
use crate::render::highlighting::highlighter;
use crate::render::state::*;
use crate::terminal::capabilities::{MarkCapability, StyleCapability, TerminalCapabilities};
use crate::terminal::osc::{clear_link, set_link_url};
use crate::terminal::TerminalSize;
use crate::theme::CombineStyle;
use crate::Theme;
use crate::{Environment, Settings};

pub fn write_indent<W: Write>(writer: &mut W, level: u16) -> Result<()> {
    write!(writer, "{}", " ".repeat(level as usize))
}

pub fn write_styled<W: Write, S: AsRef<str>>(
    writer: &mut W,
    capabilities: &TerminalCapabilities,
    style: &Style,
    text: S,
) -> Result<()> {
    match capabilities.style {
        None => write!(writer, "{}", text.as_ref()),
        Some(StyleCapability::Ansi) => write!(
            writer,
            "{}{}{}",
            style.render(),
            text.as_ref(),
            style.render_reset()
        ),
    }
}

fn write_remaining_lines<W: Write>(
    writer: &mut W,
    capabilities: &TerminalCapabilities,
    style: &Style,
    indent: u16,
    mut buffer: String,
    next_lines: &[&[Word]],
    last_line: &[Word],
) -> Result<CurrentLine> {
    // Finish the previous line
    writeln!(writer)?;
    write_indent(writer, indent)?;
    // Now write all lines up to the last
    for line in next_lines {
        match line.split_last() {
            None => {
                // The line was empty, so there's nothing to do anymore.
            }
            Some((last, heads)) => {
                for word in heads {
                    buffer.push_str(word.word);
                    buffer.push_str(word.whitespace);
                }
                buffer.push_str(last.word);
                write_styled(writer, capabilities, style, &buffer)?;
                writeln!(writer)?;
                write_indent(writer, indent)?;
                buffer.clear();
            }
        };
    }

    // Now write the last line and keep track of its width
    match last_line.split_last() {
        None => {
            // The line was empty, so there's nothing to do anymore.
            Ok(CurrentLine::empty())
        }
        Some((last, heads)) => {
            for word in heads {
                buffer.push_str(word.word);
                buffer.push_str(word.whitespace);
            }
            buffer.push_str(last.word);
            write_styled(writer, capabilities, style, &buffer)?;
            Ok(CurrentLine {
                length: textwrap::core::display_width(&buffer) as u16,
                trailing_space: Some(last.whitespace.to_owned()),
            })
        }
    }
}

pub fn write_styled_and_wrapped<W: Write, S: AsRef<str>>(
    writer: &mut W,
    capabilities: &TerminalCapabilities,
    style: &Style,
    max_width: u16,
    indent: u16,
    current_line: CurrentLine,
    text: S,
) -> Result<CurrentLine> {
    let words = WordSeparator::UnicodeBreakProperties
        .find_words(text.as_ref())
        .collect::<Vec<_>>();
    match words.first() {
        // There were no words in the text so we just do nothing.
        None => Ok(current_line),
        Some(first_word) => {
            let current_width = current_line.length
                + indent
                + current_line
                    .trailing_space
                    .as_ref()
                    .map_or(0, |s| display_width(s.as_ref()) as u16);

            // If the current line is not empty and we can't even add the first first word of the text to it
            // then lets finish the line and start over.  If the current line is empty the word simply doesn't
            // fit into the terminal size so we must print it anyway.
            if 0 < current_line.length
                && max_width < current_width + display_width(first_word) as u16
            {
                writeln!(writer)?;
                write_indent(writer, indent)?;
                return write_styled_and_wrapped(
                    writer,
                    capabilities,
                    style,
                    max_width,
                    indent,
                    CurrentLine::empty(),
                    text,
                );
            }

            let widths = [
                // For the first line we need to subtract the length of the current line, and
                // the trailing space we need to add if we add more words to this line
                (max_width - current_width.min(max_width)) as f64,
                // For remaining lines we only need to account for the indent
                (max_width - indent) as f64,
            ];
            let lines = textwrap::wrap_algorithms::wrap_first_fit(&words, &widths);
            match lines.split_first() {
                None => {
                    // there was nothing to wrap so we continue as before
                    Ok(current_line)
                }
                Some((first_line, tails)) => {
                    let mut buffer = String::with_capacity(max_width as usize);

                    // Finish the current line
                    let new_current_line = match first_line.split_last() {
                        None => {
                            // The first line was empty, so there's nothing to do anymore.
                            current_line
                        }
                        Some((last, heads)) => {
                            if let Some(s) = current_line.trailing_space {
                                buffer.push_str(&s);
                            }
                            for word in heads {
                                buffer.push_str(word.word);
                                buffer.push_str(word.whitespace);
                            }
                            buffer.push_str(last.word);
                            let length =
                                current_line.length + textwrap::core::display_width(&buffer) as u16;
                            write_styled(writer, capabilities, style, &buffer)?;
                            buffer.clear();
                            CurrentLine {
                                length,
                                trailing_space: Some(last.whitespace.to_owned()),
                            }
                        }
                    };

                    // Now write the rest of the lines
                    match tails.split_last() {
                        None => {
                            // There are no more lines and we're done here.
                            //
                            // We arrive here when the text fragment we wrapped above was
                            // shorter than the max length of the current line, i.e. we're
                            // still continuing with the current line.
                            Ok(new_current_line)
                        }
                        Some((last_line, next_lines)) => write_remaining_lines(
                            writer,
                            capabilities,
                            style,
                            indent,
                            buffer,
                            next_lines,
                            last_line,
                        ),
                    }
                }
            }
        }
    }
}

pub fn write_mark<W: Write>(writer: &mut W, capabilities: &TerminalCapabilities) -> Result<()> {
    if let Some(mark) = capabilities.marks {
        match mark {
            MarkCapability::ITerm2(marks) => marks.set_mark(writer),
        }
    } else {
        Ok(())
    }
}

pub fn write_rule<W: Write>(
    writer: &mut W,
    capabilities: &TerminalCapabilities,
    theme: &Theme,
    length: u16,
) -> std::io::Result<()> {
    let rule = "\u{2550}".repeat(length as usize);
    write_styled(
        writer,
        capabilities,
        &Style::new().fg_color(Some(theme.rule_color)),
        rule,
    )
}

pub fn write_code_block_border<W: Write>(
    writer: &mut W,
    theme: &Theme,
    capabilities: &TerminalCapabilities,
    terminal_size: &TerminalSize,
) -> std::io::Result<()> {
    let separator = "\u{2500}".repeat(terminal_size.columns.min(20) as usize);
    write_styled(
        writer,
        capabilities,
        &Style::new().fg_color(Some(theme.code_block_border_color)),
        separator,
    )?;
    writeln!(writer)
}

pub fn write_link_refs<W: Write>(
    writer: &mut W,
    environment: &Environment,
    capabilities: &TerminalCapabilities,
    links: Vec<LinkReferenceDefinition>,
) -> Result<()> {
    if !links.is_empty() {
        writeln!(writer)?;
        for link in links {
            write_styled(
                writer,
                capabilities,
                &link.style,
                format!("[{}]: ", link.index),
            )?;

            // If we can resolve the link try to write it as inline link to make the URL
            // clickable.  This mostly helps images inside inline links which we had to write as
            // reference links because we can't nest inline links.
            if let Some(url) = environment.resolve_reference(&link.target) {
                match &capabilities.style {
                    Some(StyleCapability::Ansi) => {
                        set_link_url(writer, url, &environment.hostname)?;
                        write_styled(writer, capabilities, &link.style, link.target)?;
                        clear_link(writer)?;
                    }
                    None => write_styled(writer, capabilities, &link.style, link.target)?,
                };
            } else {
                write_styled(writer, capabilities, &link.style, link.target)?;
            }

            if !link.title.is_empty() {
                write_styled(
                    writer,
                    capabilities,
                    &link.style,
                    format!(" {}", link.title),
                )?;
            }
            writeln!(writer)?;
        }
    };
    Ok(())
}

pub fn write_start_code_block<W: Write>(
    writer: &mut W,
    settings: &Settings,
    indent: u16,
    style: Style,
    block_kind: CodeBlockKind<'_>,
) -> Result<StackedState> {
    write_indent(writer, indent)?;
    write_code_block_border(
        writer,
        &settings.theme,
        &settings.terminal_capabilities,
        &settings.terminal_size,
    )?;
    // And start the indent for the contents of the block
    write_indent(writer, indent)?;

    match (&settings.terminal_capabilities.style, block_kind) {
        (Some(StyleCapability::Ansi), CodeBlockKind::Fenced(name)) if !name.is_empty() => {
            match settings.syntax_set.find_syntax_by_token(&name) {
                None => Ok(LiteralBlockAttrs {
                    indent,
                    style: settings.theme.code_style.on_top_of(&style),
                }
                .into()),
                Some(syntax) => {
                    let parse_state = ParseState::new(syntax);
                    let highlight_state = HighlightState::new(highlighter(), ScopeStack::new());
                    Ok(HighlightBlockAttrs {
                        indent,
                        highlight_state,
                        parse_state,
                    }
                    .into())
                }
            }
        }
        (_, _) => Ok(LiteralBlockAttrs {
            indent,
            style: settings.theme.code_style.on_top_of(&style),
        }
        .into()),
    }
}

pub fn write_start_heading<W: Write>(
    writer: &mut W,
    capabilities: &TerminalCapabilities,
    style: Style,
    level: HeadingLevel,
) -> Result<StackedState> {
    write_styled(
        writer,
        capabilities,
        &style,
        "\u{2504}".repeat(level as usize),
    )?;

    // Headlines never wrap, so indent doesn't matter
    Ok(StackedState::Inline(
        InlineState::InlineBlock,
        InlineAttrs { style, indent: 0 },
    ))
}

fn calculate_column_widths(table: &CurrentTable) -> Option<Vec<usize>> {
    let logical_columns = table
        .head
        .iter()
        .chain(table.rows.iter())
        .chain(table.footer.iter())
        .map(|row| row.cells.iter().map(|cell| cell.colspan.max(1)).sum::<usize>())
        .max()?;
    let mut widths = vec![0; logical_columns];
    let rows = table
        .head
        .iter()
        .chain(table.rows.as_slice())
        .chain(table.footer.as_slice());
    for row in rows {
        let mut column_index = 0usize;
        for cell in &row.cells {
            let content_width = cell
                .fragments
                .join("")
                .lines()
                .map(display_width)
                .max()
                .unwrap_or(0);
            let colspan = cell.colspan.max(1);
            if colspan == 1 {
                widths[column_index] = max(widths[column_index], content_width);
            } else {
                let current_width: usize = widths[column_index..column_index + colspan].iter().sum();
                if current_width < content_width {
                    widths[column_index + colspan - 1] += content_width - current_width;
                }
            }
            column_index += colspan;
        }
    }
    Some(widths)
}

// TODO: Support themes for table rule.
fn write_table_rule<W: Write>(
    writer: &mut W,
    capabilities: &TerminalCapabilities,
    length: u16,
) -> Result<()> {
    let rule = "\u{2500}".repeat(length.into());
    write_styled(writer, capabilities, &Style::new(), rule)?;
    writeln!(writer)
}

fn format_table_cell_line(content: &str, width: usize, alignment: Alignment) -> String {
    use Alignment::*;
    match alignment {
        Left | None => format!(" {:<width$} ", content),
        Center => format!(" {:^width$} ", content),
        Right => format!(" {:>width$} ", content),
    }
}

fn table_cell_lines(cell: &TableCell) -> Vec<String> {
    let joined = cell.fragments.join("");
    if joined.is_empty() {
        vec![String::new()]
    } else {
        joined.lines().map(ToOwned::to_owned).collect()
    }
}

pub fn write_table<W: Write>(
    writer: &mut W,
    capabilities: &TerminalCapabilities,
    terminal_size: &TerminalSize,
    table: CurrentTable,
) -> Result<()> {
    if let Some(widths) = calculate_column_widths(&table) {
        // Calculate length of the table rule.
        let total_width: usize = widths.iter().sum();
        let rule_length = min(
            // We use two spaces for padding for each cell in format_table_cell.
            (total_width + 2 * widths.len())
                .try_into()
                .unwrap_or(u16::MAX),
            terminal_size.columns,
        );
        write_table_rule(writer, capabilities, rule_length)?;

        // Write the table head in bold if any.
        if let Some(head) = table.head {
            let lines = head
                .cells
                .iter()
                .map(table_cell_lines)
                .collect::<Vec<_>>();
            let row_height = lines.iter().map(Vec::len).max().unwrap_or(1);
            for line_index in 0..row_height {
                let mut column_index = 0usize;
                for (cell_lines, cell) in zip(lines.iter(), &head.cells) {
                    let colspan = cell.colspan.max(1);
                    let width: usize =
                        widths[column_index..column_index + colspan].iter().sum::<usize>()
                            + 2 * (colspan - 1);
                    let alignment = table.alignments[column_index];
                    let content = cell_lines.get(line_index).map_or("", String::as_str);
                    write_styled(
                        writer,
                        capabilities,
                        &Style::new().bold(),
                        format_table_cell_line(content, width, alignment),
                    )?;
                    column_index += colspan;
                }
                writeln!(writer)?;
            }
            write_table_rule(writer, capabilities, rule_length)?;
        }

        // Write table body.
        for row in table.rows {
            let lines = row.cells.iter().map(table_cell_lines).collect::<Vec<_>>();
            let row_height = lines.iter().map(Vec::len).max().unwrap_or(1);
            for line_index in 0..row_height {
                let mut column_index = 0usize;
                for (cell_lines, cell) in zip(lines.iter(), &row.cells) {
                    let colspan = cell.colspan.max(1);
                    let width: usize =
                        widths[column_index..column_index + colspan].iter().sum::<usize>()
                            + 2 * (colspan - 1);
                    let alignment = table.alignments[column_index];
                    let content = cell_lines.get(line_index).map_or("", String::as_str);
                    write_styled(
                        writer,
                        capabilities,
                        &Style::new(),
                        format_table_cell_line(content, width, alignment),
                    )?;
                    column_index += colspan;
                }
                writeln!(writer)?;
            }
        }
        if !table.footer.is_empty() {
            write_table_rule(writer, capabilities, rule_length)?;
        }
        for row in table.footer {
            let lines = row.cells.iter().map(table_cell_lines).collect::<Vec<_>>();
            let row_height = lines.iter().map(Vec::len).max().unwrap_or(1);
            for line_index in 0..row_height {
                let mut column_index = 0usize;
                for (cell_lines, cell) in zip(lines.iter(), &row.cells) {
                    let colspan = cell.colspan.max(1);
                    let width: usize =
                        widths[column_index..column_index + colspan].iter().sum::<usize>()
                            + 2 * (colspan - 1);
                    let alignment = table.alignments[column_index];
                    let content = cell_lines.get(line_index).map_or("", String::as_str);
                    write_styled(
                        writer,
                        capabilities,
                        &Style::new(),
                        format_table_cell_line(content, width, alignment),
                    )?;
                    column_index += colspan;
                }
                writeln!(writer)?;
            }
        }
        write_table_rule(writer, capabilities, rule_length)?;
    }
    // Do nothing when there are no rows in the table, which should be impossible.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_cell_lines_preserves_multiline_content() {
        let cell = TableCell {
            fragments: vec!["alpha\nbeta".into()],
            colspan: 1,
        };

        assert_eq!(table_cell_lines(&cell), vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn calculate_column_widths_uses_longest_line_per_cell() {
        let table = CurrentTable {
            head: None,
            rows: vec![crate::render::data::TableRow {
                cells: vec![TableCell {
                    fragments: vec!["a\nlonger".into()],
                    colspan: 1,
                }],
                current_cell: TableCell::empty(),
            }],
            footer: Vec::new(),
            current_row: crate::render::data::TableRow::empty(),
            alignments: vec![Alignment::Left],
            current_section: crate::render::data::TableSection::Body,
        };

        assert_eq!(calculate_column_widths(&table), Some(vec![6]));
    }

    #[test]
    fn write_table_separates_footer_rows_with_a_rule() {
        let table = CurrentTable {
            head: None,
            rows: vec![crate::render::data::TableRow {
                cells: vec![TableCell {
                    fragments: vec!["body".into()],
                    colspan: 1,
                }],
                current_cell: TableCell::empty(),
            }],
            footer: vec![crate::render::data::TableRow {
                cells: vec![TableCell {
                    fragments: vec!["footer".into()],
                    colspan: 1,
                }],
                current_cell: TableCell::empty(),
            }],
            current_row: crate::render::data::TableRow::empty(),
            alignments: vec![Alignment::Left],
            current_section: crate::render::data::TableSection::Body,
        };

        let mut output = Vec::new();
        write_table(
            &mut output,
            &TerminalCapabilities::default(),
            &TerminalSize::default(),
            table,
        )
        .unwrap();

        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains(" body "));
        assert!(rendered.contains(" footer "));
        assert!(rendered.matches('─').count() >= 2);
    }

    #[test]
    fn calculate_column_widths_respects_colspan_cells() {
        let table = CurrentTable {
            head: None,
            rows: vec![crate::render::data::TableRow {
                cells: vec![
                    TableCell {
                        fragments: vec!["wide-cell".into()],
                        colspan: 2,
                    },
                    TableCell {
                        fragments: vec!["tail".into()],
                        colspan: 1,
                    },
                ],
                current_cell: TableCell::empty(),
            }],
            footer: Vec::new(),
            current_row: crate::render::data::TableRow::empty(),
            alignments: vec![Alignment::Left, Alignment::Left, Alignment::Left],
            current_section: crate::render::data::TableSection::Body,
        };

        assert_eq!(calculate_column_widths(&table), Some(vec![0, 9, 4]));
    }
}
