use color_eyre::config::HookBuilder;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyEvent, MouseEvent};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::{FutureExt, StreamExt};
use ratatui::backend::Backend;
use ratatui::prelude::*;
use ratatui_image::protocol::StatefulProtocol;
use std::error::Error;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::view::app::{App, AppState};
use crate::view::pages::SelectedTabs;
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;

use super::ChapterPagesResponse;

pub enum Action {
    GoToSearchPage,
    Quit,
    NextTab,
    PreviousTab,
}

/// These are the events this app will listen to
#[derive(Clone)]
pub enum Events {
    Tick,
    Key(KeyEvent),
    Redraw(Box<dyn StatefulProtocol>, String),
    // Todo! maybe implement something that uses the mouse?
    Mouse(MouseEvent),
    GoToMangaPage(MangaItem),
    GoBackMangaPage,
    GoSearchPage,
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

pub fn init_error_hooks() -> color_eyre::Result<()> {
    let (panic, error) = HookBuilder::default().into_hooks();
    let panic = panic.into_panic_hook();
    let error = error.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |e| {
        let _ = restore();
        error(e)
    }))?;
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        panic(info);
    }));
    Ok(())
}

///Start app's main loop
pub async fn run_app(backend: impl Backend) -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(backend)?;

    terminal.show_cursor()?;

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
                SelectedTabs::Search => {
                    app.search_page.handle_events(event);
                }
                SelectedTabs::MangaTab => {
                    app.manga_page.as_mut().unwrap().handle_events(event);
                }
                SelectedTabs::ReaderTab => {
                    app.manga_reader_page.as_mut().unwrap().handle_events(event)
                }
            };
        }

        if let Ok(app_action) = app.global_action_rx.try_recv() {
            app.update(app_action);
        }

        if app.current_tab == SelectedTabs::Search {
            if let Ok(search_page_action) = app.search_page.local_action_rx.try_recv() {
                app.search_page.update(search_page_action);
            }
        }

        if app.current_tab == SelectedTabs::MangaTab {
            if let Some(manga_page) = app.manga_page.as_mut() {
                if let Ok(action) = manga_page.local_action_rx.try_recv() {
                    manga_page.update(action);
                }
            }
        }

        if app.current_tab == SelectedTabs::ReaderTab {
            if let Some(reader_page) = app.manga_reader_page.as_mut() {
                if let Ok(reader_action) = reader_page.local_action_rx.try_recv() {
                    reader_page.update(reader_action);
                }
            }
        }
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
                                        event_tx.send(Events::Key(key)).unwrap();
                                    }
                                },
                                crossterm::event::Event::Mouse(mouse_event) => {
                                    event_tx.send(Events::Mouse(mouse_event)).unwrap();
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
                        event_tx.send(Events::Tick).unwrap();
                    }
            }
        }
    })
}
