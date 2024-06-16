use std::error::Error;
use std::time::Duration;

use color_eyre::config::HookBuilder;
use crossterm::event::{poll, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::Backend;
use ratatui::{Frame, Terminal};
use tokio::sync::mpsc::UnboundedSender;

use crate::view::app::{App, AppState};

pub enum SearchPageActions {
    SearchManga
}

pub enum Action {
    Quit,
    Tick,
    NextTab,
    PreviousTab,
    SearchPageActions(SearchPageActions),
}

/// Initialize the terminal
pub fn init() -> std::io::Result<()> {
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(())
}

pub fn restore() -> std::io::Result<()> {
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
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

fn render_ui(f: &mut Frame<'_>, app: &mut App) {
    // let image = StatefulImage::new(None).resize(ratatui_image::Resize::Fit(None));
    // let inner = f.size().inner(&ratatui::layout::Margin {
    //     horizontal: 4,
    //     vertical: 4,
    // });

    // Render with the protocol state.
    f.render_widget(app, f.size());
}




///Start app's main loop
pub async fn run_app<B: Backend>(backend: B) -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(backend)?;

    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel::<Action>();

    let mut app = App::new(action_tx);

    let events = handle_event(app.action_tx.clone());

    while app.state == AppState::Runnning {
        terminal.draw(|f| {
            render_ui(f, &mut app);
        })?;

        if let Some(action) = action_rx.recv().await {
            app.update(action);
        }
    }
    events.abort();

    Ok(())
}
