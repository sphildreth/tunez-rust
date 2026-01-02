use ratatui::style::Color;
use std::env;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub error: Color,
    pub success: Color,
    pub background: Color,
    pub text: Color,
    pub is_color: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Gray,
            accent: Color::Magenta,
            error: Color::Red,
            success: Color::Green,
            background: Color::Reset,
            text: Color::Reset,
            is_color: true,
        }
    }
}

impl Theme {
    pub fn monochrome() -> Self {
        Self {
            primary: Color::White,
            secondary: Color::Gray,
            accent: Color::White, // No color differentiation
            error: Color::White,  // Rely on text/icon
            success: Color::White,
            background: Color::Reset,
            text: Color::Reset,
            is_color: false,
        }
    }

    pub fn from_config(name: Option<&str>) -> Self {
        // Enforce NO_COLOR standard (see no-color.org)
        if env::var("NO_COLOR").is_ok() {
            return Self::monochrome();
        }

        match name {
            Some("monochrome") => Self::monochrome(),
            _ => Self::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::from_config(None);
        assert_eq!(theme.primary, Color::Cyan);
    }

    #[test]
    fn test_monochrome_config() {
        let theme = Theme::from_config(Some("monochrome"));
        assert_eq!(theme.primary, Color::White);
    }

    // Note: Testing NO_COLOR env var is tricky in unit tests running in parallel,
    // avoiding side effects.
}
