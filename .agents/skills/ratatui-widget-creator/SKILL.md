---
name: ratatui-widget-creator
description: >-
  Use this skill when creating, modifying, or refactoring TUI components, pages,
  modals, or widgets in manga-tui. Guides through Ratatui layouts, StatefulWidgetFrame,
  Component trait implementations, event handling, async task communication, and theme styling.
---

# Ratatui Widget & Page Creator Skill

This skill guides the construction and refactoring of user interface components in `manga-tui` using `ratatui`.

---

## Architecture Overview

In `manga-tui`, UI elements are organized under `src/view/`:
- `src/view/pages/`: Full screen tabs/views (`home.rs`, `manga.rs`, `reader.rs`, `search.rs`, `feed.rs`). Registered in `src/view/pages.rs` (`SelectedPage`).
- `src/view/widgets/`: Reusable building blocks and modals (`home/`, `manga/`, `reader/`, `search/`, `feed/`).
- `src/view/widgets.rs`: Defines core traits `Component` and `StatefulWidgetFrame`.

---

## 1. Choosing the Right Abstraction

### A. Full Interactive Components (`pub trait Component`)
Use when the element manages its own event loop, async tasks, and state transitions:
```rust
use ratatui::Frame;
use ratatui::layout::Rect;
use crate::view::widgets::Component;
use crate::backend::tui::Events;

pub struct MyComponent {
    // Local state, list states, flags
}

pub enum MyComponentAction {
    Select(usize),
    Close,
}

impl Component for MyComponent {
    type Actions = MyComponentAction;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        // Draw layouts, borders, and sub-widgets
    }

    fn handle_events(&mut self, events: Events) {
        // Handle KeyCode, MouseEvent, or Tick
    }

    fn update(&mut self, action: Self::Actions) {
        // Mutate state in response to actions
    }

    fn clean_up(&mut self) {
        // Abort background tokio tasks, clear caches
    }
}
```

### B. Stateful Renderers (`pub trait StatefulWidgetFrame`)
Use for modular UI elements where state is owned by a parent page (e.g., filter panels, scroll lists):
```rust
use ratatui::Frame;
use ratatui::layout::Rect;
use crate::view::widgets::StatefulWidgetFrame;

pub struct MyWidget;
pub struct MyWidgetState {
    pub selected_index: usize,
}

impl StatefulWidgetFrame for MyWidget {
    type State = MyWidgetState;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>, state: &mut Self::State) {
        // Render using passed state
    }
}
```

---

## 2. Layout & Styling Guidelines

1. **Layout Splitting**:
   Always use `Layout::default()` with `Constraint::Percentage`, `Constraint::Length`, or `Constraint::Min`:
   ```rust
   let chunks = Layout::default()
       .direction(Direction::Vertical)
       .constraints([
           Constraint::Length(3),  // Header / Search Bar
           Constraint::Min(10),    // Main Content
           Constraint::Length(3),  // Status / Keybindings footer
       ])
       .split(area);
   ```

2. **Dialogs & Modals**:
   Use `ModalBuilder` from `src/view/widgets.rs`:
   ```rust
   let modal = ModalBuilder::new(my_popup_widget)
       .with_dimensions(ModalDimensions { width: 60, height: 40 })
       .build();
   frame.render_widget(modal, area);
   ```

3. **Colors & Keybindings**:
   - Instructions style: use `crate::global::INSTRUCTIONS_STYLE`.
   - Borders: `Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)`.
   - Avoid hardcoded raw ANSI codes; use `ratatui::style::{Color, Style, Modifier}`.

---

## 3. Keyboard & Event Handling

1. Capture events in `handle_events`:
   ```rust
   match events {
       Events::Key(key_event) => match key_event.code {
           KeyCode::Char('j') | KeyCode::Down => self.next_item(),
           KeyCode::Char('k') | KeyCode::Up => self.previous_item(),
           KeyCode::Enter => self.activate_selection(),
           KeyCode::Esc => self.close(),
           _ => {},
       },
       _ => {},
   }
   ```
2. For async background tasks (e.g., fetching details or search results), use `tokio::sync::mpsc::Sender<Events>` or `tokio::task::JoinHandle` stored in the component so it can be aborted inside `clean_up()`.

See [references/component_patterns.md](./references/component_patterns.md) for full examples.
