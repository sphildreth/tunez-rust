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

    pub fn afterdark() -> Self {
        Self {
            primary: Color::LightMagenta,
            secondary: Color::DarkGray,
            accent: Color::LightCyan,
            error: Color::LightRed,
            success: Color::LightGreen,
            background: Color::Black,
            text: Color::White,
            is_color: true,
        }
    }

    pub fn solarized() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Yellow,
            accent: Color::Magenta,
            error: Color::Red,
            success: Color::Green,
            background: Color::Reset,
            text: Color::Reset,
            is_color: true,
        }
    }

    pub fn from_config(name: Option<&str>) -> Self {
        // Enforce NO_COLOR standard (see no-color.org)
        if env::var("NO_COLOR").is_ok() {
            return Self::monochrome();
        }

        match name {
            Some("monochrome") => Self::monochrome(),
            Some("afterdark") => Self::afterdark(),
            Some("solarized") => Self::solarized(),
            Some("default") | None => Self::default(),
            Some(other) => {
                tracing::warn!("Unknown theme '{}', using default", other);
                Self::default()
            }
        }
    }

    /// Parse a theme from a string (for runtime theme switching)
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "default" => Some(Self::default()),
            "monochrome" => Some(Self::monochrome()),
            "afterdark" => Some(Self::afterdark()),
            "solarized" => Some(Self::solarized()),
            _ => None,
        }
    }

    /// Get all available theme names
    pub fn available_themes() -> &'static [&'static str] {
        &["default", "monochrome", "afterdark", "solarized"]
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

    #[test]
    fn test_afterdark_theme() {
        let theme = Theme::from_config(Some("afterdark"));
        assert_eq!(theme.primary, Color::LightMagenta);
    }

    #[test]
    fn test_solarized_theme() {
        let theme = Theme::from_config(Some("solarized"));
        assert_eq!(theme.primary, Color::Cyan);
    }

    #[test]
    fn test_unknown_theme_falls_back() {
        let theme = Theme::from_config(Some("unknown-theme"));
        assert_eq!(theme.primary, Color::Cyan); // Default
    }

    #[test]
    fn test_parse_theme() {
        assert!(Theme::parse("default").is_some());
        assert!(Theme::parse("monochrome").is_some());
        assert!(Theme::parse("afterdark").is_some());
        assert!(Theme::parse("solarized").is_some());
        assert!(Theme::parse("unknown").is_none());
    }

    #[test]
    fn test_available_themes() {
        let themes = Theme::available_themes();
        assert!(themes.contains(&"default"));
        assert!(themes.contains(&"monochrome"));
        assert!(themes.contains(&"afterdark"));
        assert!(themes.contains(&"solarized"));
    }
}
