use std::sync::Arc;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::ChapterResponse;
use crate::view::widgets::manga::ChaptersListWidget;
use crate::view::widgets::{Component, ThreadImage, ThreadProtocol};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use ratatui_image::Resize;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_widget_list::ListState;

pub enum MangaPageActions {
    ScrollChapterDown,
    ScrollChapterUp,
}

pub enum MangaPageEvents {
    FetchChapters,
    LoadChapters(Option<ChapterResponse>),
}

pub struct MangaPage {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub img_url: Option<String>,
    pub image_state: Option<ThreadProtocol>,
    global_event_tx: UnboundedSender<Events>,
    local_action_tx: UnboundedSender<MangaPageActions>,
    pub local_action_rx: UnboundedReceiver<MangaPageActions>,
    local_event_tx: UnboundedSender<MangaPageEvents>,
    local_event_rx: UnboundedReceiver<MangaPageEvents>,
    client: Arc<MangadexClient>,
    chapters: Option<Chapters>,
}

struct Chapters {
    state: tui_widget_list::ListState,
    widget: ChaptersListWidget,
}

#[allow(clippy::too_many_arguments)]
impl MangaPage {
    pub fn new(
        id: String,
        title: String,
        description: String,
        tags: Vec<String>,
        img_url: Option<String>,
        image_state: Option<ThreadProtocol>,
        global_event_tx: UnboundedSender<Events>,
        client: Arc<MangadexClient>,
    ) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaPageActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaPageEvents>();

        local_event_tx.send(MangaPageEvents::FetchChapters).unwrap();

        Self {
            id,
            title,
            description,
            tags,
            img_url,
            image_state,
            global_event_tx,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            client,
            chapters: None,
        }
    }
    fn render_cover(&mut self, area: Rect, buf: &mut Buffer) {
        match self.image_state.as_mut() {
            Some(state) => {
                let image = ThreadImage::new().resize(Resize::Fit(None));
                StatefulWidget::render(image, area, buf, state);
            }
            None => {
                Block::bordered().render(area, buf);
            }
        }
    }

    fn render_manga_information(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::default()
            .margin(1)
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)]);

        let [manga_information_area, manga_chapters_area] = layout.areas(area);

        Paragraph::new(self.description.clone())
            .block(Block::bordered())
            .wrap(Wrap { trim: true })
            .render(manga_information_area, frame.buffer_mut());

        self.render_chapters(manga_chapters_area, frame);
    }

    fn render_chapters(&mut self, area: Rect, frame: &mut Frame<'_>) {
        Block::bordered().render(area, frame.buffer_mut());

        let inner_block = area.inner(&Margin {
            horizontal: 1,
            vertical: 1,
        });

        match self.chapters.as_mut() {
            Some(chapters) => {
                StatefulWidget::render(
                    chapters.widget.clone(),
                    inner_block,
                    frame.buffer_mut(),
                    &mut chapters.state,
                );
            }
            None => {
                // Todo! show chapters are loading
            }
        }
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('j') => {
                self.local_action_tx
                    .send(MangaPageActions::ScrollChapterDown)
                    .unwrap();
            }
            KeyCode::Char('k') => {
                self.local_action_tx
                    .send(MangaPageActions::ScrollChapterUp)
                    .unwrap();
            }
            _ => {}
        }
    }

    fn scroll_chapter_down(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            chapters.state.next();
        }
    }

    fn scroll_chapter_up(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            chapters.state.previous();
        }
    }

    fn tick(&mut self) {
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaPageEvents::FetchChapters => {
                    let manga_id = self.id.clone();
                    let client = Arc::clone(&self.client);
                    let tx = self.local_event_tx.clone();
                    tokio::spawn(async move {
                        let response = client.get_manga_chapters(manga_id).await;

                        match response {
                            Ok(chapters_response) => tx
                                .send(MangaPageEvents::LoadChapters(Some(chapters_response)))
                                .unwrap(),
                            Err(_e) => tx.send(MangaPageEvents::LoadChapters(None)).unwrap(),
                        }
                    });
                }
                MangaPageEvents::LoadChapters(response) => match response {
                    Some(response) => {
                        self.chapters = Some(Chapters {
                            state: tui_widget_list::ListState::default(),
                            widget: ChaptersListWidget::from_response(&response),
                        });
                    }
                    None => self.chapters = None,
                },
            }
        }
    }
}

impl Component for MangaPage {
    type Actions = MangaPageActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)]);

        let [cover_area, information_area] = layout.areas(area);

        self.render_cover(cover_area, frame.buffer_mut());
        self.render_manga_information(information_area, frame);
    }
    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaPageActions::ScrollChapterUp => self.scroll_chapter_up(),
            MangaPageActions::ScrollChapterDown => self.scroll_chapter_down(),
        }
    }
    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Redraw(protocol, manga_id) => {
                if self.id == manga_id {
                    if let Some(img_state) = self.image_state.as_mut() {
                        img_state.inner = Some(protocol);
                    }
                }
            }
            Events::Key(key_event) => self.handle_key_events(key_event),
            _ => self.tick(),
        }
    }
}
