use std::error::Error;
use std::time::Duration;

use color_eyre::config::HookBuilder;
use crossterm::event::{
    poll, DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent,
    KeyEventKind, MouseEvent,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::{FutureExt, StreamExt};
use ratatui::backend::Backend;
use ratatui::{Frame, Terminal};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::view::app::{App, AppState};
use crate::view::pages::SelectedTabs;
use crate::view::widgets::Component;

pub enum Action {
    Quit,
    NextTab,
    PreviousTab,
}

/// These are the events this app will listen to
#[derive(Clone)]
pub enum Events {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
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
pub async fn run_app<B: Backend>(backend: B) -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(backend)?;

    let (action_tx, mut action_rx) = unbounded_channel::<Action>();

    let (event_tx, mut event_rx) = unbounded_channel::<Events>();

    terminal.show_cursor()?;

    let mut app = App::new(action_tx.clone());

    let tick_rate = std::time::Duration::from_millis(250);

    handle_events(tick_rate, event_tx);

    while app.state == AppState::Runnning {
        terminal.draw(|f| {
            app.render(f.size(), f);
        })?;

        if let Some(event) = event_rx.recv().await {
            app.handle_events(event.clone());
            if app.current_tab == SelectedTabs::Search {
                app.search_page.handle_events(event);
            }
        }

        if let Ok(app_action) = action_rx.try_recv() {
            app.update(app_action);
        }

        if let Ok(search_page_action) = app.search_page.action_rx.try_recv() {
            app.search_page.update(search_page_action);
        }
    }

    Ok(())
}

pub fn handle_events(tick_rate: Duration, event_tx: UnboundedSender<Events>) {
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
    });
}