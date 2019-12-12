use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use termion::event::Key;
use termion::input::TermRead;

pub enum Event {
    KeyInput(Key),
    Tick,
    Kafka(String)
}

/// A small event handler that wrap termion input and tick events. Each event
/// type is handled in its own thread and returned to a common `Receiver`
pub struct Events {
    rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
    input_handle: thread::JoinHandle<()>,
//    tick_handle: thread::JoinHandle<()>,
//    kafka_handle: thread::JoinHandle<()>
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub exit_key: Key,
    pub tick_rate: Duration,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            exit_key: Key::Char('q'),
            tick_rate: Duration::from_secs(1),
        }
    }
}

impl Events {
    pub fn new() -> Events {
        Events::with_config(Config::default())
    }

    pub fn with_config(config: Config) -> Events {
        let (tx, rx) = mpsc::channel();
        let input_handle = {
            let tx = tx.clone();
            thread::spawn(move || {
                let stdin = io::stdin();
                for evt in stdin.keys() {
                    match evt {
                        Ok(key) => {
                            if let Err(_) = tx.send(Event::KeyInput(key)) {
                                return;
                            }
                            if key == config.exit_key {
                                return;
                            }
                        }
                        Err(_) => {}
                    }
                }
            })
        };
        Events {
            rx,
            tx,
            input_handle,
//            tick_handle,
        }
    }

    pub fn push_kafka_event(&self, message: String) -> Result<(), failure::Error> {
        self.tx.send(Event::Kafka(message))?;
        Ok(())
    }

    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.rx.recv()
    }
}
