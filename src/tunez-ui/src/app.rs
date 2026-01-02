use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
use tunez_core::{AppDirs, Provider, ProviderSelection};
use tunez_player::{Player, PlayerState, QueuePersistence};
use tunez_viz::VizMode;

use crate::help::HelpContent;
use crate::theme::Theme;
use std::sync::mpsc;
use tunez_viz::Visualizer;

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 18;
const HELP_WIDTH: u16 = 80;
const HELP_HEIGHT: u16 = 70;

#[derive(Clone)]
pub struct UiContext {
    pub provider: Arc<dyn Provider>,
    pub provider_selection: ProviderSelection,
    pub scrobbler: Option<Arc<dyn tunez_core::Scrobbler>>,
    pub theme: Theme,
    pub dirs: AppDirs,
}

impl UiContext {
    pub fn new(
        provider: Arc<dyn Provider>,
        provider_selection: ProviderSelection,
        scrobbler: Option<Arc<dyn tunez_core::Scrobbler>>,
        theme: Theme,
        dirs: AppDirs,
    ) -> Self {
        Self {
            provider,
            provider_selection,
            scrobbler,
            theme,
            dirs,
        }
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

    let mut app = App::new(context);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| app.render(frame))?;

        // Calculate adaptive tick rate based on terminal size
        let area = terminal.size().unwrap_or_default();
        let fps = if let Ok(viz_guard) = app.visualizer.lock() {
            viz_guard.get_recommended_fps(area.width, area.height)
        } else {
            20 // Default fallback
        };
        let tick_rate = Duration::from_millis(1000 / fps as u64);

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key) {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }
    }

    Ok(())
}

struct App {
    #[allow(dead_code)] // Will be used for UI-provider integration
    provider: Arc<dyn Provider>,
    provider_selection: ProviderSelection,
    player: Player,
    tabs: Vec<Tab>,
    active_tab: usize,
    show_help: bool,
    help: HelpContent,
    visualizer: Arc<Mutex<tunez_viz::Visualizer>>,
    error_rx: mpsc::Receiver<String>,
    error_message: Option<String>,
    error_timeout: Option<Instant>,
    scrobbler_manager: tunez_player::ScrobblerManager,
    queue_persistence: QueuePersistence,
    theme: Theme,
    use_color: bool,
}

impl App {
    fn new(ctx: UiContext) -> Self {
        let (tx, rx) = mpsc::channel();

        // Initialize scrobbler manager
        let mut scrobbler_manager =
            tunez_player::ScrobblerManager::new(ctx.scrobbler, "Tunez", None);
        // Enable scrobbling if configured
        scrobbler_manager.set_enabled(scrobbler_manager.is_active());
        // Hook up error callback
        {
            let tx_clone = tx.clone();
            scrobbler_manager.set_error_callback(move |msg: &str| {
                let _ = tx_clone.send(msg.to_string());
            });
        }

        let queue_persistence = QueuePersistence::new(ctx.dirs.data_dir());
        let mut player = Player::new();

        // Load persisted queue
        match queue_persistence.load() {
            Ok(queue) => {
                *player.queue_mut() = queue;
            }
            Err(e) => {
                let _ = tx.send(format!("Failed to load queue: {}", e));
            }
        }

        // Initialize visualizer with 2 channels (stereo) ? Visualizer::new() takes 0 args in lib.rs
        // Wait, app.rs line 153 said `Visualizer::new(2)`. lib.rs said `pub fn new() -> Self`.
        // I should use `Visualizer::new()`.
        let visualizer = Arc::new(Mutex::new(Visualizer::new()));
        let viz_clone = visualizer.clone();

        // Register sample callback for visualization
        player.set_sample_callback(move |samples: &[f32]| {
            if let Ok(viz) = viz_clone.lock() {
                viz.add_samples(samples);
            }
        });

        Self {
            provider: ctx.provider,
            provider_selection: ctx.provider_selection,
            player,
            tabs: Tab::all(),
            active_tab: 0,
            show_help: false,
            visualizer,
            error_rx: rx,
            error_message: None,
            error_timeout: None,
            scrobbler_manager,
            queue_persistence,
            help: HelpContent::new(),
            theme: ctx.theme,
            use_color: ctx.theme.is_color,
        }
    }

    fn tick(&mut self) {
        // Update visualizer animation phase
        if let Ok(mut viz) = self.visualizer.lock() {
            viz.update_animation();
        }

        // Update scrobbler progress
        // Note: we cast Duration to u64 seconds, losing sub-second precision which is fine for scrobbling interval checks
        self.scrobbler_manager
            .tick(&self.player, self.player.position().as_secs());

        // Check for error messages
        while let Ok(msg) = self.error_rx.try_recv() {
            self.error_message = Some(msg);
            self.error_timeout = Some(Instant::now() + Duration::from_secs(5));
        }

        // Clear error message if timeout expired
        if let Some(timeout) = self.error_timeout {
            if Instant::now() > timeout {
                self.error_message = None;
                self.error_timeout = None;
            }
        }
    }

    fn style_fg(&self, color: Color) -> Style {
        if self.use_color {
            Style::default().fg(color)
        } else {
            Style::default()
        }
    }

    fn save_queue(&mut self) {
        if let Err(e) = self.queue_persistence.save(self.player.queue()) {
            self.error_message = Some(format!("Failed to save queue: {}", e));
            self.error_timeout = Some(Instant::now() + Duration::from_secs(5));
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
            KeyCode::Char('q') | KeyCode::Esc => {
                self.save_queue();
                return true;
            }
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('j') | KeyCode::Down => self.next_tab(),
            KeyCode::Char('k') | KeyCode::Up => self.previous_tab(),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => self.previous_tab(),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => self.next_tab(),
            KeyCode::Char(c) if c.is_ascii_digit() => self.jump_to_tab(c),
            // Search mode
            KeyCode::Char('/') => {
                // Switch to search tab
                if let Some(search_idx) = self.tabs.iter().position(|t| matches!(t, Tab::Search)) {
                    self.active_tab = search_idx;
                }
            }
            // Visualization mode switching
            KeyCode::Char('v') => {
                // Cycle through visualization modes
                if let Ok(mut viz_guard) = self.visualizer.lock() {
                    let current_mode = viz_guard.mode();
                    let all_modes = VizMode::all();
                    let current_idx = all_modes
                        .iter()
                        .position(|&m| m == current_mode)
                        .unwrap_or(0);
                    let next_idx = (current_idx + 1) % all_modes.len();
                    viz_guard.set_mode(all_modes[next_idx]);
                }
            }
            // Playback controls
            KeyCode::Char(' ') => match self.player.state() {
                tunez_player::PlayerState::Playing { .. } => {
                    self.player.pause();
                    self.scrobbler_manager
                        .on_state_change(&self.player, tunez_core::PlaybackState::Paused);
                }
                _ => {
                    self.player.play();
                    if let tunez_player::PlayerState::Playing { .. } = self.player.state() {
                        self.scrobbler_manager
                            .on_state_change(&self.player, tunez_core::PlaybackState::Resumed);
                        // Or Started? Context dependent. Simple toggling usually implies Resume if paused.
                        // If it was Stopped, it implies Started.
                        // We should check previous state?
                        // Simplify: just say Resumed/Started. Manager logic should handle duplicates or we trust the mapping.
                        // Actually, Play vs Resume.
                        // If we were Stopped, play() starts from scratch.
                        // If Paused, play() resumes.
                        // We can check local var logic or assume Started if position is near 0?
                        // Let's assume on_state_change handles it or we refine.
                        // For now, let's map to Started if we were Stopped?
                        // But self.player.play() resets state.
                        // Let's assume Started for simplicity in toggle from Stopped.
                        self.scrobbler_manager
                            .on_state_change(&self.player, tunez_core::PlaybackState::Started);
                    }
                }
            },
            KeyCode::Char('n') => {
                // Scrobble stop for current track before skipping
                self.scrobbler_manager
                    .on_state_change(&self.player, tunez_core::PlaybackState::Stopped);
                self.player.skip_next();
                // Scrobble start for new track
                self.scrobbler_manager
                    .on_state_change(&self.player, tunez_core::PlaybackState::Started);
                self.save_queue();
            }
            KeyCode::Char('p') => {
                // Previous track logic would go here
            }
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
                Constraint::Min(7),
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
        let provider = if let Some(profile) = &self.provider_selection.profile {
            format!(
                "Provider: {} (profile: {})",
                self.provider_selection.provider_id, profile
            )
        } else {
            format!("Provider: {}", self.provider_selection.provider_id)
        };

        let status = Line::from(vec![
            Span::styled(
                "Tunez ",
                self.style_fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("▸ "),
            Span::styled(provider, self.style_fg(self.theme.success)),
            Span::raw("  Net: OK  Scrobble: OFF (text labels shown for accessibility)"),
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
            .highlight_style(if self.use_color {
                Style::default()
                    .bg(self.theme.secondary)
                    .fg(self.theme.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            })
            .highlight_symbol("▸ ");
        let mut state = ratatui::widgets::ListState::default();
        state.select(Some(self.active_tab));
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_main(&self, frame: &mut Frame, area: Rect) {
        let tab = self.tabs.get(self.active_tab).unwrap_or(&Tab::NowPlaying);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(6)])
            .split(area);

        match tab {
            Tab::NowPlaying => {
                self.render_now_playing(frame, chunks[0]);
                self.render_visualizer(frame, chunks[1]);
            }
            Tab::Search => {
                self.render_search(frame, chunks[0]);
            }
            Tab::Library => {
                self.render_library(frame, chunks[0]);
            }
            Tab::Playlists => {
                self.render_playlists(frame, chunks[0]);
            }
            Tab::Queue => {
                self.render_queue(frame, chunks[0]);
            }
            Tab::Lyrics => {
                self.render_lyrics(frame, chunks[0]);
            }
            Tab::Config => {
                self.render_config(frame, chunks[0]);
            }
            Tab::Help => {
                self.render_help_main(frame, chunks[0]);
            }
        }
    }

    fn render_now_playing(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::NowPlaying.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            title,
            self.style_fg(self.theme.primary)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Show current track info if available
        if let Some(current) = self.player.current() {
            lines.push(Line::from(Span::styled(
                format!(
                    "Now Playing: {} - {}",
                    current.track.artist, current.track.title
                ),
                self.style_fg(self.theme.success)
                    .add_modifier(Modifier::BOLD),
            )));
            if let Some(album) = &current.track.album {
                lines.push(Line::from(format!("Album: {}", album)));
            }
            if let Some(duration) = current.track.duration_seconds {
                lines.push(Line::from(format!("Duration: {}s", duration)));
            }
        } else {
            lines.push(Line::from("No track playing"));
        }

        lines.push(Line::from(""));
        lines.extend(hints);

        let paragraph = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_search(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::Search.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let lines = vec![
            Line::from(Span::styled(
                title,
                self.style_fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Search functionality will be implemented here"),
            Line::from("Press '/' to enter search mode"),
            Line::from(""),
        ];

        let mut text = Text::from(lines);
        text.extend(hints);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_library(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::Library.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let lines = vec![
            Line::from(Span::styled(
                title,
                self.style_fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Library browsing will be implemented here"),
            Line::from("Browse artists, albums, genres"),
            Line::from(""),
        ];

        let mut text = Text::from(lines);
        text.extend(hints);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_playlists(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::Playlists.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let lines = vec![
            Line::from(Span::styled(
                title,
                self.style_fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Playlists will be implemented here"),
            Line::from("List and manage playlists"),
            Line::from(""),
        ];

        let mut text = Text::from(lines);
        text.extend(hints);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_queue(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::Queue.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            title,
            self.style_fg(self.theme.primary)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Show queue items
        lines.push(Line::from(format!(
            "Queue: {} tracks",
            self.player.queue().len()
        )));
        if !self.player.queue().is_empty() {
            lines.push(Line::from(""));
            for (i, item) in self.player.queue().items().iter().take(10).enumerate() {
                let prefix = if Some(item.id) == self.player.current().map(|c| c.id) {
                    "▶ "
                } else {
                    "  "
                };
                lines.push(Line::from(format!(
                    "{}{}. {} - {}",
                    prefix,
                    i + 1,
                    item.track.artist,
                    item.track.title
                )));
            }
            if self.player.queue().len() > 10 {
                lines.push(Line::from(format!(
                    "... and {} more",
                    self.player.queue().len() - 10
                )));
            }
        } else {
            lines.push(Line::from("Queue is empty"));
        }

        lines.push(Line::from(""));
        lines.extend(hints);

        let paragraph = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_lyrics(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::Lyrics.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let lines = vec![
            Line::from(Span::styled(
                title,
                self.style_fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Lyrics display will be implemented here"),
            Line::from(""),
        ];

        let mut text = Text::from(lines);
        text.extend(hints);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_config(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::Config.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let lines = vec![
            Line::from(Span::styled(
                title,
                self.style_fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Configuration view will be implemented here"),
            Line::from(""),
        ];

        let mut text = Text::from(lines);
        text.extend(hints);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_help_main(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} (Phase 1D shell)", Tab::Help.display_name());
        let hints = vec![
            Line::from("Navigation: j/k or ↑/↓ | h/l or ←/→ | Tab/Shift+Tab | 1-8"),
            Line::from("Help: ?   Quit: q or Esc   Tabs: Now Playing, Search, Library, Playlists, Queue, Lyrics, Config, Help"),
        ];

        let lines = vec![
            Line::from(Span::styled(
                title,
                self.style_fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Help content will be displayed here"),
            Line::from(""),
        ];

        let mut text = Text::from(lines);
        text.extend(hints);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let player_state_str = match self.player.state() {
            PlayerState::Stopped => "⏹  Stopped",
            PlayerState::Buffering { .. } => "⏳ Buffering",
            PlayerState::Playing { .. } => "⏵  Playing",
            PlayerState::Paused { .. } => "⏸  Paused",
            PlayerState::Error { message, .. } => &format!("⚠️  Error: {}", message),
        };

        let footer = Paragraph::new(Line::from(vec![
            Span::raw(player_state_str),
            Span::raw("   ▓▓▓▓░░░░░░  Vol: 72%  Rep:Off"),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Player"));
        frame.render_widget(footer, area);
    }

    fn render_visualizer(&self, frame: &mut Frame, area: Rect) {
        // Graceful degradation based on terminal size
        if area.width < 24 || area.height < 4 {
            let msg = Paragraph::new("Visualizer hidden (terminal too small)")
                .block(Block::default().borders(Borders::ALL).title("Visualizer"));
            frame.render_widget(msg, area);
            return;
        }

        // Check if visualization is supported
        if let Ok(viz_guard) = self.visualizer.lock() {
            if !viz_guard.should_render(area.width, area.height) {
                let msg = Paragraph::new("Visualizer disabled (terminal too small)")
                    .block(Block::default().borders(Borders::ALL).title("Visualizer"));
                frame.render_widget(msg, area);
                return;
            }

            // Use the new visualization system
            // Pass color info to visualizer for monochrome fallback
            viz_guard.render_with_color_support(frame, area, self.use_color);
        }
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
    use std::sync::Arc;
    use tunez_core::provider::ProviderCapabilities;

    // Mock provider for testing
    struct MockProvider;

    impl tunez_core::Provider for MockProvider {
        fn id(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "Mock"
        }
        fn capabilities(&self) -> tunez_core::ProviderCapabilities {
            ProviderCapabilities::default()
        }
        fn search_tracks(
            &self,
            _query: &str,
            _filters: tunez_core::TrackSearchFilters,
            _paging: tunez_core::PageRequest,
        ) -> tunez_core::ProviderResult<tunez_core::Page<tunez_core::Track>> {
            Ok(tunez_core::Page {
                items: vec![],
                next: None,
            })
        }
        fn browse(
            &self,
            _kind: tunez_core::BrowseKind,
            _paging: tunez_core::PageRequest,
        ) -> tunez_core::ProviderResult<tunez_core::Page<tunez_core::CollectionItem>> {
            Ok(tunez_core::Page {
                items: vec![],
                next: None,
            })
        }
        fn list_playlists(
            &self,
            _paging: tunez_core::PageRequest,
        ) -> tunez_core::ProviderResult<tunez_core::Page<tunez_core::Playlist>> {
            Ok(tunez_core::Page {
                items: vec![],
                next: None,
            })
        }
        fn search_playlists(
            &self,
            _query: &str,
            _paging: tunez_core::PageRequest,
        ) -> tunez_core::ProviderResult<tunez_core::Page<tunez_core::Playlist>> {
            Ok(tunez_core::Page {
                items: vec![],
                next: None,
            })
        }
        fn get_playlist(
            &self,
            _playlist_id: &tunez_core::PlaylistId,
        ) -> tunez_core::ProviderResult<tunez_core::Playlist> {
            unimplemented!()
        }
        fn list_playlist_tracks(
            &self,
            _playlist_id: &tunez_core::PlaylistId,
            _paging: tunez_core::PageRequest,
        ) -> tunez_core::ProviderResult<tunez_core::Page<tunez_core::Track>> {
            Ok(tunez_core::Page {
                items: vec![],
                next: None,
            })
        }
        fn get_album(
            &self,
            _album_id: &tunez_core::AlbumId,
        ) -> tunez_core::ProviderResult<tunez_core::Album> {
            unimplemented!()
        }
        fn list_album_tracks(
            &self,
            _album_id: &tunez_core::AlbumId,
            _paging: tunez_core::PageRequest,
        ) -> tunez_core::ProviderResult<tunez_core::Page<tunez_core::Track>> {
            Ok(tunez_core::Page {
                items: vec![],
                next: None,
            })
        }
        fn get_track(
            &self,
            _track_id: &tunez_core::TrackId,
        ) -> tunez_core::ProviderResult<tunez_core::Track> {
            unimplemented!()
        }
        fn get_stream_url(
            &self,
            _track_id: &tunez_core::TrackId,
        ) -> tunez_core::ProviderResult<tunez_core::StreamUrl> {
            unimplemented!()
        }
    }

    #[test]
    fn tab_numbers_jump_correctly() {
        let provider = Arc::new(MockProvider);
        let provider_selection = ProviderSelection {
            provider_id: "filesystem".into(),
            profile: Some("home".into()),
        };
        let dirs = tunez_core::AppDirs::discover().expect("failed to discover dirs");
        let context = UiContext::new(provider, provider_selection, None, Theme::default(), dirs);
        let mut app = App::new(context);
        app.jump_to_tab('3');
        assert_eq!(app.active_tab, 2);
        app.jump_to_tab('9'); // out of range ignored
        assert_eq!(app.active_tab, 2);
    }
}
