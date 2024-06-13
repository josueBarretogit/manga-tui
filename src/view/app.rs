use std::borrow::Borrow;
use std::error::Error;
use std::io::Cursor;
use std::time::Duration;
use std::usize;

use crossterm::event::{poll, Event, KeyCode, KeyEventKind};
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::widgets::{Tabs, Widget};
use ratatui::{Frame, Terminal};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;
use strum::IntoEnumIterator;

use crate::backend::tui::Action;
use crate::view::pages::*;

pub struct App {
    pub pages: Pages,
}

impl Widget for &mut App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let main_layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)]);

        let [top_tabs_area, page_are] = main_layout.areas(area);

        self.render_top_tabs(top_tabs_area, buf);

        self.render_pages(page_are, buf);
    }
}

impl App {
    pub fn new() -> Self {
        // let mut picker = Picker::from_termios().unwrap();
        // // Guess the protocol.
        // picker.guess_protocol();
        //
        // // Load an image with the image crate.
        //
        // let dyn_img = image::io::Reader::new(Cursor::new("some".as_bytes()))
        //     .with_guessed_format()
        //     .unwrap();
        //
        // // Create the Protocol which will be used by the widget.
        // let image = picker.new_resize_protocol(dyn_img.decode().unwrap());

        App {
            pages: Pages::default(),
        }
    }

    pub fn render_top_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<String> = Pages::iter().map(|page| page.to_string()).collect();

        let current_page = self.pages.clone() as usize;

        Tabs::new(titles)
            .highlight_style(Color::Red)
            .select(current_page)
            .padding("", "")
            .divider(" | ")
            .render(area, buf);
    }

    pub fn render_pages(&self, area: Rect, buf: &mut Buffer) {
        match self.pages {
            Pages::Home => {}
            Pages::Search => self.render_search_page(area, buf),
            Pages::MangaPage => {}
        }
    }

    pub fn render_search_page(&self, area: Rect, buf: &mut Buffer) {}

    pub fn render_home_page(&self, area: Rect, buf: &mut Buffer) {}

    pub fn render_manga_page(&self, area: Rect, buf: &mut Buffer) {}
}

fn render_ui(f: &mut Frame<'_>, app: &mut App) {
    // let image = StatefulImage::new(None).resize(ratatui_image::Resize::Fit(None));
    // let inner = f.size().inner(&ratatui::layout::Margin {
    //     horizontal: 4,
    //     vertical: 4,
    // });

    // Render with the protocol state.
    f.render_widget(app, f.size());
}

fn user_actions(tick_rate: Duration) -> Action {
    if poll(tick_rate).unwrap() {
        if let Event::Key(key) = crossterm::event::read().unwrap() {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => Action::Quit,
                    KeyCode::Up => Action::ZoomIn,
                    KeyCode::Down => Action::ZoomOut,
                    _ => Action::Tick,
                }
            } else {
                Action::Tick
            }
        } else {
            Action::Tick
        }
    } else {
        Action::Tick
    }
}

///Start app's main loop
pub async fn run_app<B: Backend>(backend: B) -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    loop {
        terminal.draw(|f| {
            render_ui(f, &mut app);
        })?;

        let action = user_actions(Duration::from_millis(250));

        match action {
            Action::Quit => break,
            _ => continue,
        }
    }
    Ok(())
}
