#![deny(unused_mut)]
extern crate zookeeper;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate termion;

mod events;


use structopt::StructOpt;
use zookeeper::{Acl, CreateMode, Watcher, WatchedEvent, ZooKeeper, ZkError};
use std::time::Duration;
use failure::Error;
use std::io;
use termion::raw::IntoRawMode;
use termion::input::MouseTerminal;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::Terminal;
use tui::layout::{Layout, Direction, Constraint, Corner};
use tui::style::{Style, Color, Modifier};
use tui::widgets::{Borders, SelectableList, Block, Widget, Text, List};
use std::io::stdin;
use termion::event::Key;
use events::{Events, Event};
use termion::color;
use termion::screen::*;

enum Message {
    Error(String),
    Value(String, String)
}


struct LoggingWatcher;

impl Watcher for LoggingWatcher {
    fn handle(&self, e: WatchedEvent) {
        info!("{:?}", e)
    }
}

fn create_term_data(zk: &ZooKeeper) -> Result<(), Error> {
    let test = zk.create("/term", "Valeur de base".as_bytes().to_vec(), Acl::open_unsafe().clone(), CreateMode::Persistent)?;
    info!("Creation : {}", test);
    let test = zk.create("/term/data1", "Valeur de noeud1".as_bytes().to_vec(), Acl::open_unsafe().clone(), CreateMode::Persistent)?;
    info!("Creation : {}", test);
    let test = zk.create("/term/data2", "Valeur de noeud2".as_bytes().to_vec(), Acl::open_unsafe().clone(), CreateMode::Persistent)?;
    info!("Creation : {}", test);
    Ok(())
}


fn run() -> Result<(), Error> {
    let zk_urls = "localhost:2181";
    info!("connecting to {}", zk_urls);

    let zk = ZooKeeper::connect("localhost:2181", Duration::from_secs(15), LoggingWatcher)?;

    let term_exists = zk.exists("/term", false)?;
    if term_exists.is_none() { create_term_data(&zk)? }

    let (data, _) = zk.get_data("/term", false)?;
    let data_string = String::from_utf8(data)?;

    let events = Events::new();

    let stdin = stdin();
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut path: Vec<String> = vec![];
    let mut selected = Some(0);

    let mut messages: Vec<Message> = vec![];

    let default = "/".to_string();
    let mut children = zk.get_children(&(default.clone()+path.join("/").as_str()), false)?;

    loop {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(f.size());

            let style = Style::default().fg(Color::White);
//                .bg(Color::Black);
            SelectableList::default()
                .block(Block::default().borders(Borders::ALL).title(&(default.clone()+path.join("/").as_str())))
                .items(&children)
                .select(selected)
                .style(style)
                .highlight_style(style.fg(Color::LightGreen).modifier(Modifier::BOLD))
                .highlight_symbol(">")
                .render(&mut f, chunks[0]);
            {
                let events = messages.iter().map(|msg|
                    match msg {
                        Message::Error(err) => Text::styled(err, Style::default().fg(Color::Red)),
                        Message::Value(node, data) => Text::styled(format!("{} => {}", node, data), Style::default().fg(Color::LightBlue))
                    }
                );
//                let events = vec![Text::styled("Coucou".to_string(), Style::default().fg(Color::Red))];
                List::new(events.into_iter())
                    .block(Block::default().borders(Borders::ALL).title("Errors"))
                    .start_corner(Corner::BottomLeft)
                    .render(&mut f, chunks[1]);
            }
        })?;

        match events.next()? {
            Event::KeyInput(input) => match input {
                Key::Char('q') => break,
                Key::Down => selected = selected.map(|value| value + 1).filter(|value| *value < children.len()).or(Some(0)),
                Key::Up => selected = selected.and_then(|value| if value == 0 { None } else { Some(value - 1) }).filter(|value| *value >= 0).or(Some(children.len() - 1)),
                Key::Left => {
                    path.pop();
                    match zk.get_children(&(default.clone()+path.join("/").as_str()), false)
                        .and_then(|childs| if childs.len() < 1 { Err(ZkError::NoNode) } else { Ok(childs) }) {
                        Ok(nodes) => {
                            children = nodes;
                            selected = Some(0);
                        }
                        Err(e) =>  messages.push(Message::Error(format!("{:?}", e)))
                    }
                },
                Key::Right => {
                    if selected.is_some() {
                        path.push(children[selected.unwrap()].clone());
                        match zk.get_children(&(default.clone()+path.join("/").as_str()), false)
                            .and_then(|childs| if childs.len() < 1 { Err(ZkError::NoNode) } else { Ok(childs) }) {
                            Ok(nodes) => {
                                children = nodes;
                                selected = Some(0);
                            }
                            Err(e) =>  messages.push(Message::Error(format!("{:?}", e)))
                        }
                    }
                },
                Key::Char('\n') => {
                    if selected.is_some() {
                        let mut selected_path = default.clone()+path.join("/").as_str()+"/"+children[selected.unwrap()].as_str();
                        if selected_path.starts_with("//") { selected_path.remove(0); }
                        match zk.get_data(&selected_path, false)
                            .map_err(|err| err.into())
                            .and_then(|(data, _)| String::from_utf8(data).map_err(|err| Error::from(err)) ) {
                            Ok(data) => messages.push(Message::Value(selected_path, data)),
                            Err(e) => messages.push(Message::Error(format!("{:?}", e)))
                        }
                    } else {
                        match zk.get_data(&(default.clone()+path.join("/").as_str()), false)
                            .map_err(|err| err.into())
                            .and_then(|(data, _)| String::from_utf8(data).map_err(|err| Error::from(err)) ) {
                            Ok(data) => messages.push(Message::Value(default.clone()+path.join("/").as_str(), data)),
                            Err(e) => messages.push(Message::Error(format!("{:?}", e)))
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(())
}

fn main() {
    env_logger::init();
    match run() {
        Err(e) => error!("Error : {}", e),
        _ => println!("No error ! ")
    }
}
