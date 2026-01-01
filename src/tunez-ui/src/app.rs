use std::io::stdout;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use thiserror::Error;
use tunez_core::ProviderSelection;

use crate::help::HelpContent;

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 18;
const HELP_WIDTH: u16 = 80;
const HELP_HEIGHT: u16 = 70;
const TICK_RATE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub struct UiContext {
    pub provider: ProviderSelection,
}

impl UiContext {
    pub fn new(provider: ProviderSelection) -> Self {
        Self { provider }
    }
}

#[derive(Debug, Error)]
pub enum UiError {
    #[error("terminal error: {0}")]
    Io(#[from] std::io::Error),
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, UiError> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}

pub fn run_ui(context: UiContext) -> Result<(), UiError> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(context.provider);

    loop {
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key) {
                    break;
                }
            }
        }
    }

    Ok(())
}

struct App {
    provider: ProviderSelection,
    tabs: Vec<Tab>,
    active_tab: usize,
    show_help: bool,
    help: HelpContent,
}

impl App {
    fn new(provider: ProviderSelection) -> Self {
        Self {
            provider,
            tabs: Tab::all(),
            active_tab: 0,
            show_help: false,
            help: HelpContent::new(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('j') | KeyCode::Down => self.next_tab(),
            KeyCode::Char('k') | KeyCode::Up => self.previous_tab(),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => self.previous_tab(),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => self.next_tab(),
            KeyCode::Char(c) if c.is_ascii_digit() => self.jump_to_tab(c),
            _ => {}
        }
        false
    }

    fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
    }

    fn previous_tab(&mut self) {
        if self.active_tab == 0 {
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.active_tab -= 1;
        }
    }

    fn jump_to_tab(&mut self, digit: char) {
        if let Some(index) = digit.to_digit(10) {
            if index == 0 {
                return;
            }
            let idx = (index - 1) as usize;
            if idx < self.tabs.len() {
                self.active_tab = idx;
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.size();
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            let message = format!(
                "Resize terminal to at least {MIN_WIDTH}x{MIN_HEIGHT} (current: {}x{})",
                area.width, area.height
            );
            let paragraph = Paragraph::new(message)
                .wrap(Wrap { trim: true })
                .block(Block::default().title("Tunez").borders(Borders::ALL));
            frame.render_widget(paragraph, area);
            return;
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_header(frame, layout[0]);
        self.render_body(frame, layout[1]);
        self.render_footer(frame, layout[2]);

        if self.show_help {
            self.render_help(frame, area);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let provider = if let Some(profile) = &self.provider.profile {
            format!(
                "Provider: {} (profile: {})",
                self.provider.provider_id, profile
            )
        } else {
            format!("Provider: {}", self.provider.provider_id)
        };

        let status = Line::from(vec![
            Span::styled(
                "Tunez ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("▸ "),
            Span::styled(provider, Style::default().fg(Color::Green)),
            Span::raw("  Net: OK  Scrobble: OFF"),
        ]);

        let paragraph = Paragraph::new(status)
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_body(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(18), Constraint::Min(10)])
            .split(area);

        self.render_nav(frame, chunks[0]);
        self.render_main(frame, chunks[1]);
    }

    fn render_nav(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .tabs
            .iter()
            .map(|tab| ListItem::new(tab.display_name()))
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Tabs"))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");
        let mut state = ratatui::widgets::ListState::default();
        state.select(Some(self.active_tab));
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_main(&self, frame: &mut Frame, area: Rect) {
        let tab = self.tabs.get(self.active_tab).unwrap_or(&Tab::NowPlaying);
        let title = format!("{} (Phase 1D shell)", tab.display_name());
        let description = tab.description();
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];
        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.extend(description);
        lines.push(Line::from(""));
        lines.extend(hints);

        let paragraph = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer = Paragraph::new(Line::from(vec![
            Span::raw("⏵  "),
            Span::styled(
                "Not playing",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM),
            ),
            Span::raw("   ▓▓▓▓░░░░░░  Vol: 72%  Rep:Off"),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Player"));
        frame.render_widget(footer, area);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(HELP_WIDTH, HELP_HEIGHT, area);
        let help_text = self.help.text();
        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title("Help — Keys (press ? to close)")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(Clear, popup_area);
        frame.render_widget(help, popup_area);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    NowPlaying,
    Search,
    Library,
    Playlists,
    Queue,
    Lyrics,
    Config,
    Help,
}

impl Tab {
    fn all() -> Vec<Tab> {
        vec![
            Tab::NowPlaying,
            Tab::Search,
            Tab::Library,
            Tab::Playlists,
            Tab::Queue,
            Tab::Lyrics,
            Tab::Config,
            Tab::Help,
        ]
    }

    fn display_name(&self) -> &'static str {
        match self {
            Tab::NowPlaying => "Now Playing",
            Tab::Search => "Search",
            Tab::Library => "Library",
            Tab::Playlists => "Playlists",
            Tab::Queue => "Queue",
            Tab::Lyrics => "Lyrics",
            Tab::Config => "Config",
            Tab::Help => "Help",
        }
    }

    fn description(&self) -> Vec<Line<'static>> {
        match self {
            Tab::NowPlaying => vec![Line::from(
                "Now Playing dashboard placeholder — playback wiring arrives in later phases.",
            )],
            Tab::Search => vec![Line::from(
                "Search view placeholder — results and provider-backed queries arrive in later phases.",
            )],
            Tab::Library => vec![Line::from(
                "Library browse placeholder — provider-driven navigation arrives in later phases.",
            )],
            Tab::Playlists => vec![Line::from(
                "Playlists placeholder — listing and opening playlists will be added later.",
            )],
            Tab::Queue => vec![Line::from(
                "Queue placeholder — queue management and playback ordering arrive in Phase 1E.",
            )],
            Tab::Lyrics => vec![Line::from(
                "Lyrics placeholder — scrolling lyrics rendering arrives after provider support.",
            )],
            Tab::Config => vec![Line::from(
                "Config placeholder — editable configuration UI will land with config UX work.",
            )],
            Tab::Help => vec![Line::from(
                "Press ? to view the Markdown-driven help overlay.",
            )],
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(horizontal[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_numbers_jump_correctly() {
        let provider = ProviderSelection {
            provider_id: "filesystem".into(),
            profile: Some("home".into()),
        };
        let mut app = App::new(provider);
        app.jump_to_tab('3');
        assert_eq!(app.active_tab, 2);
        app.jump_to_tab('9'); // out of range ignored
        assert_eq!(app.active_tab, 2);
    }
}
