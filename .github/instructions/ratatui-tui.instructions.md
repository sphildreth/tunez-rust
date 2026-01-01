---
description: 'Ratatui terminal user interface (TUI) development best practices for Tunez music player'
applyTo: '**/tunez-ui/**/*.rs,**/tui/**/*.rs,**/*_ui.rs,**/*_tui.rs'
---

# Ratatui TUI Development Guidelines for Tunez

## Project Context

Tunez is a keyboard-first terminal music player with rich, animated UI. The TUI is built with [ratatui](https://ratatui.rs/) and must be:
- **Responsive**: Never block on network/audio operations
- **Keyboard-first**: All functionality accessible via keyboard
- **Cross-platform**: Consistent behavior on Linux/macOS/Windows terminals
- **Accessible**: Support monochrome/low-color terminals
- **Efficient**: Adaptive rendering with graceful degradation

Canonical reference: `docs/tunez-tui-mockups.md`

## Core Principles

### 1. Frame Structure and Layout
- Use consistent layout regions across screens:
  - **Top Status Bar**: Provider, network, scrobble status, clock
  - **Left Navigation**: Tab list with current selection indicator
  - **Main Pane**: Context-specific content (lists, details, lyrics)
  - **Bottom Player Bar**: Now playing, progress, controls, volume
- Use `Layout::default()` with `Direction::Vertical` and `Direction::Horizontal` constraints
- Prefer `Constraint::Percentage` for responsive layouts, `Constraint::Length` for fixed UI elements
- Always render within the provided `Frame` bounds; never assume terminal size

### 2. Rendering Performance
- Keep render functions pure and fast (target <16ms per frame for 60 FPS)
- Use `StatefulWidget` for components with internal state (e.g., scrollable lists)
- Minimize allocations in render functions; prefer `&str` and borrowing
- Use `Block::default().borders(Borders::ALL)` for visual hierarchy
- Implement adaptive FPS based on terminal capabilities:
  ```rust
  // Example: Reduce visualizer resolution on slow terminals
  if frame_time > TARGET_FRAME_TIME {
      visualizer_bars = min(visualizer_bars, FALLBACK_BAR_COUNT);
  }
  ```

### 3. Event Handling
- Use `crossterm` for input events (consistent with ratatui)
- Never block the event loop; use `poll()` with timeouts
- Implement debouncing for rapid key repeats on search/filter
- Structure event handlers by screen/view:
  ```rust
  match event {
      Event::Key(key) => match self.current_screen {
          Screen::NowPlaying => self.handle_now_playing_input(key),
          Screen::Search => self.handle_search_input(key),
          // ...
      },
      Event::Tick => self.update_animations(),
  }
  ```

### 4. Keyboard Navigation
- Follow vim-style navigation patterns where appropriate:
  - `j`/`k` or `↓`/`↑` for list navigation
  - `g`/`G` for first/last item
  - `h`/`l` or `←`/`→` for horizontal navigation
  - `/` for search
  - `?` for help overlay
- Always provide `Esc` or `q` to exit context/overlay
- Display keybinding hints in status bar or overlays
- Support both arrow keys AND vim bindings for accessibility

### 5. State Management
- Keep UI state separate from application state
- Use message-passing (channels) to communicate with audio/network subsystems
- Never call blocking operations in the render thread:
  ```rust
  // ❌ BAD: Blocking in UI thread
  fn render(&mut self, frame: &mut Frame) {
      let tracks = self.provider.search_blocking("query"); // BLOCKS!
  }
  
  // ✅ GOOD: Async via channels
  fn handle_search(&mut self, query: String) {
      self.tx.send(Command::Search(query)).ok();
      self.ui_state.show_loading = true;
  }
  ```
- Maintain scroll state, selection indices, and cursor positions in dedicated UI state structs

### 6. Theming and Colors
- Support multiple themes (reference: AfterDark theme in mockups)
- Use semantic color names (Accent, Dim, Warn, Error) not hardcoded RGB
- Gracefully degrade to 16-color or monochrome terminals:
  ```rust
  let style = if supports_true_color {
      Style::default().fg(Color::Rgb(138, 43, 226))
  } else {
      Style::default().fg(Color::Magenta)
  };
  ```
- Use `Style::default()` as base and chain modifiers for clarity
- Highlight current selection with inverted colors or distinct background

### 7. Animation and Progress
- Use smooth, non-blocking progress bars:
  ```rust
  Gauge::default()
      .block(Block::default())
      .gauge_style(Style::default().fg(Color::Cyan))
      .ratio(self.progress / self.total_duration)
  ```
- Implement loading spinners for async operations (rotate frames: `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`)
- Update visualizer (spectrum/waveform) at configurable FPS (default 30, adaptive down to 10)
- Avoid flashy animations; prefer subtle transitions

### 8. Text Rendering and Truncation
- Truncate long text with ellipsis (`…`) to fit terminal width
- Use `textwrap` or manual wrapping for multi-line text (lyrics, descriptions)
- Center-align titles, left-align lists and metadata
- Handle Unicode correctly (account for grapheme clusters, not bytes):
  ```rust
  use unicode_width::UnicodeWidthStr;
  let display_width = text.width();
  ```

### 9. Overlays and Popups
- Render help overlay (`?`) on top of current screen without clearing it
- Use `Popup` or `Clear` widget to avoid background artifacts
- Provide clear dismissal instructions ("Press Esc to close")
- Size popups proportionally (e.g., 80% width, 60% height) using `Rect::centered()`

### 10. Error Display
- Show non-blocking error messages in status bar or temporary overlay
- Use distinct color (e.g., `Color::Red` or `Style::default().fg(Color::Red)`)
- Auto-dismiss informational messages after timeout (e.g., 3 seconds)
- Keep critical errors visible until user acknowledges

### 11. Lists and Tables
- Use `List` widget for simple item navigation
- Use `Table` for multi-column data (track lists with artist, album, duration)
- Implement virtual scrolling for large datasets (only render visible items)
- Highlight selected row with `Style::default().bg(Color::DarkGray)`
- Show scroll indicators when content exceeds viewport

### 12. Accessibility
- Ensure all information is readable in monochrome mode
- Provide alternative representations for visualizer (e.g., text-based meter)
- Never rely solely on color to convey state; use text labels
- Support screen reader friendly output (avoid excessive animation)

## Testing TUI Components

### Unit Tests
- Test layout calculations with fixed terminal sizes
- Verify state transitions (navigation, selection)
- Mock `Frame` for render function tests

### Manual Testing
- Test on multiple terminal emulators (Windows Terminal, iTerm2, Alacritty, Gnome Terminal)
- Verify with different `TERM` values (xterm-256color, xterm, screen)
- Test with various terminal sizes (80×24, 120×40, 200×60)
- Validate with `NO_COLOR=1` environment variable
- Check resize behavior (handle `Event::Resize`)

## Common Patterns

### Pattern: Stateful List with Scroll
```rust
use ratatui::widgets::{List, ListState};

struct SelectableList {
    items: Vec<String>,
    state: ListState,
}

impl SelectableList {
    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => (i + 1) % self.items.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }
    
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<_> = self.items.iter()
            .map(|i| ListItem::new(i.as_str()))
            .collect();
        
        let list = List::new(items)
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("▸ ");
        
        frame.render_stateful_widget(list, area, &mut self.state);
    }
}
```

### Pattern: Non-Blocking Command Processing
```rust
enum Command {
    Search(String),
    Play(TrackId),
    NextTrack,
}

enum Response {
    SearchResults(Vec<Track>),
    PlaybackStarted,
    Error(String),
}

// In main loop
loop {
    // Handle UI events
    if let Ok(event) = rx_events.try_recv() {
        app.handle_event(event);
    }
    
    // Process responses from backend
    while let Ok(response) = rx_responses.try_recv() {
        app.handle_response(response);
    }
    
    // Render
    terminal.draw(|f| app.render(f))?;
}
```

## Anti-Patterns (Avoid These)

❌ **Blocking the UI thread:**
```rust
// BAD: Network call in render
fn render(&self, frame: &mut Frame) {
    let data = reqwest::blocking::get("https://api...").unwrap(); // BLOCKS!
}
```

❌ **Ignoring terminal capabilities:**
```rust
// BAD: Assumes 256-color support
let color = Color::Rgb(138, 43, 226);
```

❌ **Excessive allocations in render loop:**
```rust
// BAD: Creates new strings every frame
fn render(&self, frame: &mut Frame) {
    for i in 0..1000 {
        let text = format!("Item {}", i); // Allocation per iteration!
    }
}
```

❌ **Hardcoded terminal dimensions:**
```rust
// BAD: Assumes 80×24 terminal
let area = Rect::new(0, 0, 80, 24);
```

## References

- [Ratatui Documentation](https://ratatui.rs/)
- [Crossterm Documentation](https://docs.rs/crossterm/)
- TUI Mockups: `docs/tunez-tui-mockups.md`
- Tunez PRD: `docs/tunez-requirements.md` (Section 5: TUI requirements)
- Unicode Width: [`unicode-width` crate](https://docs.rs/unicode-width/)
