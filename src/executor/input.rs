use crossterm::event;

use crate::events::AppEvent;

pub fn start_input_capture(event_tx: tokio::sync::broadcast::Sender<AppEvent>) {
    tokio::spawn(async move {
        loop {
            let event = event::read();
            match event {
                Ok(e) => match e {
                    event::Event::Key(key_event) => {
                        let _ = event_tx.send(AppEvent::KeyPress(key_event));
                    }
                    _ => {}
                },
                Err(err) => {}
            }
        }
    });
}
