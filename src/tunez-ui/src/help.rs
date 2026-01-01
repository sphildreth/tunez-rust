use ratatui::text::{Line, Text};

const HELP_MD: &str = include_str!("help/help.md");

#[derive(Debug, Clone)]
pub struct HelpContent {
    lines: Vec<Line<'static>>,
}

impl HelpContent {
    pub fn new() -> Self {
        Self {
            lines: parse_markdown(HELP_MD),
        }
    }

    pub fn text(&self) -> Text<'static> {
        Text::from(self.lines.clone())
    }
}

fn parse_markdown(markdown: &str) -> Vec<Line<'static>> {
    markdown.lines().map(parse_line).collect()
}

fn parse_line(line: &str) -> Line<'static> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Line::from("");
    }

    if let Some(content) = trimmed.strip_prefix("### ") {
        return Line::from(content.to_string());
    }

    if let Some(content) = trimmed.strip_prefix("## ") {
        return Line::from(content.to_string());
    }

    if let Some(content) = trimmed.strip_prefix("# ") {
        return Line::from(content.to_uppercase());
    }

    if let Some(content) = trimmed.strip_prefix("- ") {
        return Line::from(format!("• {content}"));
    }

    Line::from(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headings_and_bullets() {
        let lines = parse_markdown("# Title\n## Section\n- item");
        assert_eq!(lines[0], Line::from("TITLE"));
        assert_eq!(lines[1], Line::from("Section"));
        assert_eq!(lines[2], Line::from("• item"));
    }
}
