# Ratatui Component Patterns in manga-tui

### 1. Scrollable List with Selection State

```rust
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;
use ratatui::layout::Rect;

pub struct ScrollableListWidget<'a> {
    items: Vec<String>,
    title: &'a str,
}

pub struct ScrollableListState {
    pub list_state: ListState,
}

impl ScrollableListState {
    pub fn new(count: usize) -> Self {
        let mut state = ListState::default();
        if count > 0 {
            state.select(Some(0));
        }
        Self { list_state: state }
    }

    pub fn next(&mut self, total: usize) {
        if total == 0 { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= total - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self, total: usize) {
        if total == 0 { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { total - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}
```

---

### 2. Centered Confirmation Popup

```rust
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::layout::Rect;
use crate::utils::centered_rect;

pub fn render_confirmation_popup(frame: &mut Frame<'_>, area: Rect, message: &str) {
    let popup_area = centered_rect(50, 25, area);
    // Clear area behind modal to avoid bleed-through
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Action ");
    let text = Paragraph::new(message)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(text, popup_area);
}
```

---

### 3. Asynchronous Task Lifecycle with Clean-up

Always store task handles so they can be safely aborted when switching tabs:

```rust
pub struct PageComponent {
    search_task: Option<tokio::task::JoinHandle<()>>,
}

impl Component for PageComponent {
    // ...
    fn clean_up(&mut self) {
        if let Some(task) = self.search_task.take() {
            task.abort();
        }
    }
}
```
