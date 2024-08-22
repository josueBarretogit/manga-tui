use std::error::Error;
use std::time::Duration;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyEvent, MouseEvent};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use futures::{FutureExt, StreamExt};
use ratatui::backend::Backend;
use ratatui::Terminal;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use super::ChapterPagesResponse;
use crate::common::{Artist, Author};
use crate::view::app::{App, AppState};
use crate::view::pages::SelectedPage;
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;

pub enum Action {
    Quit,
}

/// These are the events this app will listen to
#[derive(Clone)]
pub enum Events {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    GoToMangaPage(MangaItem),
    GoToHome,
    GoSearchPage,
    GoSearchMangasAuthor(Author),
    GoSearchMangasArtist(Artist),
    GoFeedPage,
    ReadChapter(ChapterPagesResponse),
}

/// Initialize the terminal
pub fn init() -> std::io::Result<()> {
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;
    Ok(())
}

pub fn restore() -> std::io::Result<()> {
    execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    Ok(())
}

///Start app's main loop
pub async fn run_app(backend: impl Backend) -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let tick_rate = std::time::Duration::from_millis(250);

    let main_event_handle = handle_events(tick_rate, app.global_event_tx.clone());

    while app.state == AppState::Runnning {
        terminal.draw(|f| {
            app.render(f.size(), f);
        })?;

        if let Some(event) = app.global_event_rx.recv().await {
            app.handle_events(event.clone());
            match app.current_tab {
                SelectedPage::Search => {
                    app.search_page.handle_events(event);
                },
                SelectedPage::MangaTab => {
                    app.manga_page.as_mut().unwrap().handle_events(event);
                },
                SelectedPage::ReaderTab => {
                    app.manga_reader_page.as_mut().unwrap().handle_events(event);
                },
                SelectedPage::Home => {
                    app.home_page.handle_events(event);
                },
                SelectedPage::Feed => {
                    app.feed_page.handle_events(event);
                },
            };
        }

        if let Ok(app_action) = app.global_action_rx.try_recv() {
            app.update(app_action);
        }

        match app.current_tab {
            SelectedPage::Search => {
                if let Ok(search_page_action) = app.search_page.local_action_rx.try_recv() {
                    app.search_page.update(search_page_action);
                }
            },
            SelectedPage::MangaTab => {
                if let Some(manga_page) = app.manga_page.as_mut() {
                    if let Ok(action) = manga_page.local_action_rx.try_recv() {
                        manga_page.update(action);
                    }
                }
            },
            SelectedPage::ReaderTab => {
                if let Some(reader_page) = app.manga_reader_page.as_mut() {
                    if let Ok(reader_action) = reader_page.local_action_rx.try_recv() {
                        reader_page.update(reader_action);
                    }
                }
            },
            SelectedPage::Home => {
                if let Ok(home_action) = app.home_page.local_action_rx.try_recv() {
                    app.home_page.update(home_action);
                }
            },
            SelectedPage::Feed => {
                if let Ok(feed_event) = app.feed_page.local_action_rx.try_recv() {
                    app.feed_page.update(feed_event);
                }
            },
        };
    }

    main_event_handle.abort();

    Ok(())
}

pub fn handle_events(tick_rate: Duration, event_tx: UnboundedSender<Events>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = crossterm::event::EventStream::new();
        let mut tick_interval = tokio::time::interval(tick_rate);

        loop {
            let delay = tick_interval.tick();
            let event = reader.next().fuse();
            tokio::select! {
                maybe_event = event => {
                    match maybe_event  {
                        Some(Ok(evt)) => {
                            match evt {
                                crossterm::event::Event::Key(key) => {
                                    if key.kind == crossterm::event::KeyEventKind::Press {
                                        event_tx.send(Events::Key(key)).ok();
                                    }
                                },
                                crossterm::event::Event::Mouse(mouse_event) => {
                                    event_tx.send(Events::Mouse(mouse_event)).ok();
                                }
                                _ => {}
                            }
                        }
                        Some(Err(_)) => {

                        }
                        None => {}

                    }

                }
                    _ = delay => {
                        event_tx.send(Events::Tick).ok();
                    }
            }
        }
    })
}
