use std::io::Cursor;
use std::time::Duration;

use crossterm::event::{poll, Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::{terminal::Frame, Terminal};
use ratatui_image::{picker::Picker, StatefulImage};

use self::backend::tui::{init, restore, Action};
use self::view::app::App;

mod backend;
/// These would be like the frontend
mod view;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let test_image_endpoint = "https://cmdxd98sb0x3yprd.mangadex.network/data/e5f224ce785f745c17a7e4edbca0673e/x1-f3d501b76b57c66f17262478ca4c1bb0ced3e2fd92de3ea49f0aa581a3619e84.jpg";

    let fetched_image_bytes = reqwest::get(test_image_endpoint)
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();

    init()?;

    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // Should use Picker::from_termios(), to get the font size,
    // but we can't put that here because that would break doctests!
    let mut picker = Picker::from_termios()?;
    // Guess the protocol.
    picker.guess_protocol();

    // Load an image with the image crate.

    let dyn_img = image::io::Reader::new(Cursor::new(fetched_image_bytes))
        .with_guessed_format()
        .unwrap();

    // Create the Protocol which will be used by the widget.
    let image = picker.new_resize_protocol(dyn_img.decode().unwrap());

    let mut app = App {
        image,
        image_width: 50,
        image_heigth: 20,
    };

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let action = user_actions(Duration::from_millis(250));

        match action {
            Action::Quit => break,
            Action::ZoomIn => app.zoom_in(),
            Action::ZoomOut => app.zoom_out(),
            _ => continue,
        }
    }

    restore()?;
    Ok(())
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let image = StatefulImage::new(None).resize(ratatui_image::Resize::Crop);

    let inner = f.size();

    // Render with the protocol state.
    f.render_stateful_widget(image, inner, &mut app.image);
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
