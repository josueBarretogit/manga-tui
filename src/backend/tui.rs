use std::error::Error;
use std::time::Duration;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyEvent, MouseEvent};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use futures::{FutureExt, StreamExt};
use ratatui::backend::Backend;
use ratatui::Terminal;
use ratatui_image::picker::{Picker, ProtocolType};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use super::fetch::ApiClient;
use super::tracker::MangaTracker;
use crate::common::{Artist, Author};
use crate::view::app::{App, AppState, MangaToRead};
use crate::view::pages::reader::{ChapterToRead, SearchChapter, SearchMangaPanel};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;

pub enum Action {
    Quit,
}

/// These are the events this app will listen to
#[derive(Clone, Debug, PartialEq)]
pub enum Events {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    GoToMangaPage(MangaItem),
    GoBackMangaPage,
    GoToHome,
    GoSearchPage,
    GoSearchMangasAuthor(Author),
    GoSearchMangasArtist(Artist),
    GoFeedPage,
    ReadChapter(ChapterToRead, MangaToRead),
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

#[cfg(unix)]
fn get_picker() -> Option<Picker> {
    Picker::from_termios()
        .ok()
        .map(|mut picker| {
            picker.guess_protocol();
            picker
        })
        .filter(|picker| picker.protocol_type != ProtocolType::Halfblocks)
}

#[cfg(target_os = "windows")]
fn get_picker() -> Option<Picker> {
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;

    struct FontSize {
        pub width: u16,
        pub height: u16,
    }
    impl Default for FontSize {
        fn default() -> Self {
            FontSize {
                width: 17,
                height: 38,
            }
        }
    }

    let size: FontSize = match unsafe { GetDpiForWindow(GetConsoleWindow()) } {
        96 => FontSize {
            width: 9,
            height: 20,
        },
        120 => FontSize {
            width: 12,
            height: 25,
        },
        144 => FontSize {
            width: 14,
            height: 32,
        },
        _ => FontSize::default(),
    };

    let mut picker = Picker::new((size.width, size.height));

    let protocol = picker.guess_protocol();

    if protocol == ProtocolType::Halfblocks {
        return None;
    }
    Some(picker)
}

///Start app's main loop
pub async fn run_app(
    backend: impl Backend,
    api_client: impl ApiClient + SearchChapter + SearchMangaPanel,
    manga_tracker: Option<impl MangaTracker>,
) -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(api_client, manga_tracker, get_picker());

    let tick_rate = std::time::Duration::from_millis(250);

    let main_event_handle = handle_events(tick_rate, app.global_event_tx.clone());

    while app.state == AppState::Runnning {
        terminal.draw(|f| {
            app.render(f.size(), f);
        })?;

        app.listen_to_event().await;

        app.update_based_on_action();
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
