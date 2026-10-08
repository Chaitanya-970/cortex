//! Terminal-native Markdown rendering and code block syntax highlighting.

use crate::theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Keywords recognized for lightweight syntax highlighting across common programming languages.
const KEYWORDS: &[&str] = &[
    "fn",
    "pub",
    "struct",
    "enum",
    "impl",
    "trait",
    "let",
    "mut",
    "const",
    "type",
    "def",
    "class",
    "import",
    "from",
    "return",
    "if",
    "else",
    "elif",
    "match",
    "case",
    "for",
    "while",
    "loop",
    "in",
    "as",
    "async",
    "await",
    "function",
    "var",
    "interface",
    "export",
    "default",
    "try",
    "catch",
    "finally",
    "throw",
    "yield",
    "package",
    "select",
    "switch",
    "break",
    "continue",
    "self",
    "this",
    "super",
    "where",
    "use",
    "mod",
];

/// Highlight a single line of code within a fenced code block.
fn highlight_code_line(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let trimmed = line.trim_start();
    let leading_spaces = line.len() - trimmed.len();
    if leading_spaces > 0 {
        spans.push(Span::raw(" ".repeat(leading_spaces)));
    }

    // Check for comment line
    if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("--") {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default()
                .fg(theme::COLOR_MUTED)
                .add_modifier(Modifier::DIM),
        ));
        return spans;
    }

    // Tokenize line by whitespace and common punctuation delimiters
    let mut current_token = String::new();
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // String literals
        if ch == '"' || ch == '\'' {
            if !current_token.is_empty() {
                spans.push(Span::styled(
                    current_token.clone(),
                    Style::default().fg(theme::COLOR_FG),
                ));
                current_token.clear();
            }

            let quote = ch;
            let mut str_literal = String::new();
            str_literal.push(quote);
            i += 1;
            while i < chars.len() {
                str_literal.push(chars[i]);
                if chars[i] == quote && chars.get(i.saturating_sub(1)) != Some(&'\\') {
                    i += 1;
                    break;
                }
                i += 1;
            }
            spans.push(Span::styled(
                str_literal,
                Style::default().fg(theme::COLOR_SUCCESS),
            ));
            continue;
        }

        // Delimiters / Punctuation
        if ch.is_whitespace() || "(),;:{}[]<>=+-*/&|!.".contains(ch) {
            if !current_token.is_empty() {
                if KEYWORDS.contains(&current_token.as_str()) {
                    spans.push(Span::styled(
                        current_token.clone(),
                        Style::default()
                            .fg(theme::COLOR_LAVENDER)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if current_token
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '_')
                {
                    spans.push(Span::styled(
                        current_token.clone(),
                        Style::default().fg(theme::COLOR_WARNING),
                    ));
                } else if current_token
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    spans.push(Span::styled(
                        current_token.clone(),
                        Style::default().fg(theme::COLOR_CLAUDE_SHIMMER),
                    ));
                } else {
                    spans.push(Span::styled(
                        current_token.clone(),
                        Style::default().fg(theme::COLOR_FG),
                    ));
                }
                current_token.clear();
            }

            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(theme::COLOR_MUTED),
            ));
            i += 1;
            continue;
        }

        current_token.push(ch);
        i += 1;
    }

    if !current_token.is_empty() {
        if KEYWORDS.contains(&current_token.as_str()) {
            spans.push(Span::styled(
                current_token,
                Style::default()
                    .fg(theme::COLOR_LAVENDER)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                current_token,
                Style::default().fg(theme::COLOR_FG),
            ));
        }
    }

    spans
}

/// Parse inline spans supporting backtick code `` `foo` `` and bold `**bar**`.
fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        // Look for inline backticks
        if let Some(start_tick) = remaining.find('`') {
            if start_tick > 0 {
                let prefix = &remaining[..start_tick];
                parse_bold_spans(prefix, &mut spans);
            }

            let after_first = &remaining[start_tick + 1..];
            if let Some(end_tick) = after_first.find('`') {
                let code_content = &after_first[..end_tick];
                spans.push(Span::styled(
                    format!(" {} ", code_content),
                    Style::default()
                        .fg(theme::COLOR_CLAUDE_SHIMMER)
                        .bg(theme::COLOR_SURFACE)
                        .add_modifier(Modifier::BOLD),
                ));
                remaining = &after_first[end_tick + 1..];
            } else {
                spans.push(Span::styled(
                    remaining.to_string(),
                    Style::default().fg(theme::COLOR_FG),
                ));
                break;
            }
        } else {
            parse_bold_spans(remaining, &mut spans);
            break;
        }
    }

    spans
}

fn parse_bold_spans(text: &str, spans: &mut Vec<Span<'static>>) {
    let mut remaining = text;
    while let Some(start_star) = remaining.find("**") {
        if start_star > 0 {
            spans.push(Span::styled(
                remaining[..start_star].to_string(),
                Style::default().fg(theme::COLOR_FG),
            ));
        }

        let after_start = &remaining[start_star + 2..];
        if let Some(end_star) = after_start.find("**") {
            let bold_content = &after_start[..end_star];
            spans.push(Span::styled(
                bold_content.to_string(),
                Style::default()
                    .fg(theme::COLOR_FG)
                    .add_modifier(Modifier::BOLD),
            ));
            remaining = &after_start[end_star + 2..];
        } else {
            spans.push(Span::styled(
                remaining.to_string(),
                Style::default().fg(theme::COLOR_FG),
            ));
            return;
        }
    }

    if !remaining.is_empty() {
        spans.push(Span::styled(
            remaining.to_string(),
            Style::default().fg(theme::COLOR_FG),
        ));
    }
}

/// Render a Markdown document into a list of terminal [`Line`] widgets.
pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();

        // Fenced code block toggle
        if trimmed.starts_with("```") {
            if in_code_block {
                lines.push(Line::from(Span::styled(
                    "    └─────────────────────────────────────────────────────────────────────────",
                    Style::default().fg(theme::COLOR_SECONDARY),
                )));
                in_code_block = false;
                code_lang.clear();
            } else {
                code_lang = trimmed.trim_start_matches('`').trim().to_string();
                let lang_label = if code_lang.is_empty() {
                    "Code"
                } else {
                    &code_lang
                };
                lines.push(Line::from(Span::styled(
                    format!(
                        "    ┌─ {} ─────────────────────────────────────────────────────────",
                        lang_label
                    ),
                    Style::default().fg(theme::COLOR_SECONDARY),
                )));
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            let mut spans = vec![Span::styled(
                "    │ ",
                Style::default().fg(theme::COLOR_SECONDARY),
            )];
            spans.extend(highlight_code_line(raw_line));
            lines.push(Line::from(spans));
            continue;
        }

        // Headings
        if let Some(h1) = trimmed.strip_prefix("# ") {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    h1.to_string(),
                    Style::default()
                        .fg(theme::COLOR_PRIMARY)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
            ]));
            continue;
        } else if let Some(h2) = trimmed.strip_prefix("## ") {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    h2.to_string(),
                    Style::default()
                        .fg(theme::COLOR_FG)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            continue;
        } else if let Some(h3) = trimmed.strip_prefix("### ") {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    h3.to_string(),
                    Style::default()
                        .fg(theme::COLOR_LAVENDER)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            continue;
        }

        // Bullet lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let item_text = &trimmed[2..];
            let mut spans = vec![
                Span::raw("    "),
                Span::styled("• ", Style::default().fg(theme::COLOR_LAVENDER)),
            ];
            spans.extend(parse_inline_spans(item_text));
            lines.push(Line::from(spans));
            continue;
        }

        // Numbered lists
        if let Some(dot_idx) = trimmed.find(". ") {
            if dot_idx > 0 && trimmed[..dot_idx].chars().all(|c| c.is_ascii_digit()) {
                let num_str = &trimmed[..=dot_idx];
                let item_text = &trimmed[dot_idx + 2..];
                let mut spans = vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{} ", num_str),
                        Style::default().fg(theme::COLOR_LAVENDER),
                    ),
                ];
                spans.extend(parse_inline_spans(item_text));
                lines.push(Line::from(spans));
                continue;
            }
        }

        // Blockquote
        if let Some(quote) = trimmed.strip_prefix("> ") {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled("│ ", Style::default().fg(theme::COLOR_LAVENDER)),
                Span::styled(
                    quote.to_string(),
                    Style::default()
                        .fg(theme::COLOR_MUTED)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
            continue;
        }

        // Empty line
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        // Regular paragraph text
        let mut spans = vec![Span::raw("    ")];
        spans.extend(parse_inline_spans(raw_line));
        lines.push(Line::from(spans));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_headings_and_code() {
        let md = r#"# Main Header
Here is some text with `inline_code()` and **bold**.
```rust
fn main() {
    let message = "hello";
}
```
- Item 1
- Item 2
"#;
        let lines = render_markdown(md);
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|l| l.to_string().contains("Main Header")));
        assert!(lines
            .iter()
            .any(|l| l.to_string().contains("inline_code()")));
        assert!(lines.iter().any(|l| l.to_string().contains("fn main")));
        assert!(lines.iter().any(|l| l.to_string().contains("• Item 1")));
    }
}
