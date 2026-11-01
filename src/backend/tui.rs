use std::error::Error;
use std::time::Duration;

use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use futures::{FutureExt, StreamExt};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui_image::picker::{Picker, ProtocolType};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use super::manga_provider::{ChapterToRead, Manga, MangaProvider};
use super::tracker::MangaTracker;
use crate::view::app::{App, AppState, MangaToRead};
use crate::view::widgets::Component;

pub enum Action {
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewChapterNotificationGroup {
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_numbers: String,
    pub chapters: Vec<super::manga_provider::LatestChapter>,
}

/// These are the events this app will listen to
#[derive(Clone, Debug, PartialEq)]
pub enum Events {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    GoToMangaPage(Manga),
    GoBackMangaPage,
    GoToHome,
    GoSearchPage,
    GoFeedPage,
    Error(String),
    CheckForNewDownloadedChapters,
    NewChaptersNotification(Vec<NewChapterNotificationGroup>),
    NewChaptersDownloadFinished(Result<usize, String>),
    AutoDownloadedFavorites {
        count: usize,
        details: Vec<String>,
    },
    ReadChapter(ChapterToRead, MangaToRead),
    EnqueueDownloadBatch(super::download_queue::EnqueueDownloadBatch),
    DownloadQueueProgress(super::download_queue::DownloadQueueProgress),
    DownloadQueueFinished(usize),
    DownloadQueueChapterFinished {
        manga_id: String,
        chapter_id: String,
    },
    SearchAllProviders(String),
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
/// Default font width for Windows console on 96 DPI
const DEFAULT_FONT_SIZE_WIDTH: u16 = 10;
#[cfg(target_os = "windows")]
/// Default font height for Windows console on 96 DPI
const DEFAULT_FONT_SIZE_HEIGHT: u16 = 22;

#[cfg(target_os = "windows")]
fn get_picker() -> Option<Picker> {
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;

    let dpi = unsafe { GetDpiForWindow(GetConsoleWindow()) };
    // In some terminals (ConPTY, Windows Terminal) there is no classic console window and the dpi is 0
    let dpi = if dpi == 0 { 96 } else { dpi };
    let scale = dpi as f32 / 96.0;
    let size: (u16, u16) = (
        ((DEFAULT_FONT_SIZE_WIDTH as f32 * scale).round() as u16).max(1),
        ((DEFAULT_FONT_SIZE_HEIGHT as f32 * scale).round() as u16).max(1),
    );

    let mut picker = Picker::new((size.0, size.1));

    let protocol = picker.guess_protocol();

    if protocol == ProtocolType::Halfblocks {
        return None;
    }
    Some(picker)
}

///Start app's main loop
pub async fn run_app<T: MangaProvider>(
    mut terminal: Terminal<impl Backend>,
    api_client: T,
    manga_tracker: Option<impl MangaTracker>,
    filter_state: T::FiltersHandler,
    filter_widget: T::Widget,
) -> Result<(), Box<dyn Error>> {
    let mut app = App::new(api_client, manga_tracker, get_picker(), filter_state, filter_widget);
    app.check_for_new_downloaded_chapters();

    let tick_rate = std::time::Duration::from_millis(250);

    let main_event_handle = handle_events(tick_rate, app.global_event_tx.clone());

    while app.state == AppState::Runnning {
        terminal.draw(|f| {
            app.render(f.area(), f);
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
                    match maybe_event {
                        Some(Ok(evt)) => {
                            let sent = match evt {
                                crossterm::event::Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                                    event_tx.send(Events::Key(key)).is_ok()
                                },
                                // Mouse movement / drag events are discarded, only clicks and scroll are relevant
                                crossterm::event::Event::Mouse(mouse_event)
                                    if !matches!(mouse_event.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) =>
                                {
                                    event_tx.send(Events::Mouse(mouse_event)).is_ok()
                                },
                                // Redraw immediately so images and layouts adapt to the new size
                                crossterm::event::Event::Resize(_, _) => event_tx.send(Events::Tick).is_ok(),
                                _ => true,
                            };
                            if !sent {
                                break;
                            }
                        },
                        Some(Err(_)) => {},
                        // The event stream is finished, polling it again would return `None` immediately
                        // causing a busy loop
                        None => break,
                    }
                },
                _ = delay => {
                    if event_tx.send(Events::Tick).is_err() {
                        break;
                    }
                }
            }
        }
    })
}
