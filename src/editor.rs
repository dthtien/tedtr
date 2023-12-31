use termion::event::Key;
use termion::color;

use std::env;
use std::time::Duration;
use std::time::Instant;

use crate::Document;
use crate::Row;
use crate::Terminal;


const VERSION: &str = env!("CARGO_PKG_VERSION");
const STATUS_BG_COLOR: color::Rgb = color::Rgb(239, 239, 239);
const STATUS_FG_COLOR: color::Rgb = color::Rgb(63, 63, 63);
const QUIT_TIMES: u8 = 3;


#[derive(Default)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

struct StatusMessage {
    text: String,
    time: Instant,
}

impl StatusMessage {
    fn from(message: String) -> Self {
        Self {
            time: Instant::now(),
            text: message,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Visual,
}

pub struct Editor {
    should_quit: bool,
    terminal: Terminal,
    current_position: Position,
    document: Document,
    offset: Position,
    status_message: StatusMessage,
    mode: Mode,
    quit_times: u8,
}

impl Editor {
    pub fn default() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut initial_status = String::from("HELP: Ctrl-S = save | Ctrl-Q = quit");

        let document = if let Some(file_name) = args.get(1) {
            let doc = Document::open(file_name);
            if let Ok(doc) = doc {
                doc
            } else {
                initial_status = format!("ERR: Could not open file: {}", file_name);
                Document::default()
            }
        } else {
            Document::default()
        };

        Self {
            should_quit: false,
            terminal: Terminal::default().expect("Failed to initialize terminal"),
            document,
            current_position: Position::default(),
            offset: Position::default(),
            status_message: StatusMessage::from(initial_status),
            mode: Mode::Normal,
            quit_times: QUIT_TIMES,
        }
    }

    #[allow(clippy::unused_self)]
    pub fn run(&mut self) {
        loop {
            if let Err(e) = self.refresh_screen() {
                die(e);
            }
            if let Err(e) = self.process_keypress() {
                die(e);
            }
            if self.should_quit {
                break;
            }
        }
    }

    fn prompt(&mut self, prompt: &str) -> Result<Option<String>, std::io::Error> {
        let mut result = String::new();
        loop {
            self.status_message = StatusMessage::from(format!("{}{}", prompt, result));
            self.refresh_screen()?;
            match Terminal::read_key()? {
                Key::Backspace => result.truncate(result.len().saturating_sub(1)),
                Key::Char('\n') => break,
                Key::Char(c) => {
                    if !c.is_control() {
                        result.push(c);
                    }
                },
                Key::Esc => {
                    result.truncate(0);
                },
                _ => (),
            }
        }
        self.status_message = StatusMessage::from(String::new());
        if result.is_empty() {
            return Ok(None);
        }
        Ok(Some(result))
    }

    fn save(&mut self) {
        if self.document.file_name.is_none() {
            let new_name = self.prompt("Save as: ").unwrap_or(None);
            if new_name.is_none() {
                self.status_message = StatusMessage::from(String::from("Save aborted"));
                return;
            }
            self.document.file_name = new_name;
        }
    }

    fn process_keypress(&mut self) -> Result<(), std::io::Error> {
        let pressed_key = Terminal::read_key()?;
        match pressed_key {
            Key::Ctrl('q') => {
                if self.quit_times > 0 && self.document.is_dirty() {
                    self.status_message = StatusMessage::from(format!(
                        "WARNING! File has unsaved changes. Press Ctrl-Q {} more times to quit.",
                        self.quit_times
                    ));
                    self.quit_times -= 1;
                    return Ok(());

                }

                self.should_quit = true;
            },
            Key::Ctrl('s') => self.save(),
            Key::Esc => self.mode = Mode::Normal,
            // Key::Char(c) => {
                // if self.mode == Mode::Insert {
                    // self.insert(c);
                // } else {
                    // match c {
                        // 'h' | 'j' | 'k' | 'l' => {
                            // if self.mode == Mode::Normal {
                                // self.move_cursor(pressed_key);
                            // } else {
                                // self.insert(c);
                            // }
                        // },
                        // 'i' => self.mode = Mode::Insert,
                        // 'v' => self.mode = Mode::Visual,
                        // _ => (),
                    // }
                // }
            // },
            Key::Char(c) => {
                self.document.insert(&self.current_position, c);
                self.move_cursor(Key::Right);
            },
            Key::Delete => self.document.delete(&self.current_position),
            Key::Backspace => {
                if self.current_position.x > 0 || self.current_position.y > 0 {
                    self.document.delete(&self.current_position);
                    self.move_cursor(Key::Left);
                    self.document.delete(&self.current_position);
                }
            },
            Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::PageUp
            | Key::PageDown
            | Key::End
            | Key::Home => self.move_cursor(pressed_key),
            _ => (),
        }
        self.scroll();
        if self.quit_times < QUIT_TIMES {
            self.quit_times = QUIT_TIMES;
            self.status_message = StatusMessage::from(String::new());
        }
        Ok(())
    }

    fn _insert(&mut self, c: char) {
        self.document.insert(&self.current_position, c);
        self.move_cursor(Key::Right);
    }

    fn scroll(&mut self) {
        let Position { x, y } = self.current_position;
        let width = self.terminal.size().width as usize;
        let height = self.terminal.size().height as usize;
        let mut offset = &mut self.offset;

        if y < offset.y {
            offset.y = y;
        } else if y >= offset.y.saturating_add(height) {
            offset.y = y.saturating_sub(height).saturating_add(1);
        }

        if x < offset.x {
            offset.x = x;
        } else if x >= offset.x.saturating_add(width) {
            offset.x = x.saturating_sub(width).saturating_add(1);
        }
    }

    fn draw_welcome_message(&self) {
        let mut welcome_message = format!("Tedtr -- version {} - dthtien", VERSION);
        let width = self.terminal.size().width as usize;
        let len = welcome_message.len();
        #[allow(clippy::arithmetic_side_effects, clippy::integer_division)]
        let padding = width.saturating_sub(len) / 2;
        let spaces = " ".repeat(padding.saturating_sub(1));
        welcome_message = format!("~{}{}", spaces, welcome_message);
        welcome_message.truncate(width);
        println!("{}\r", welcome_message);
    }

    fn move_cursor(&mut self, key: Key) {
        let terminal_height = self.terminal.size().height as usize;
        let Position { mut x, mut y } = self.current_position;
        let height = self.document.len();
        let mut width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };
        match key {
            Key::Up => y = y.saturating_sub(1),
            Key::Char('k') => {
                if self.mode == Mode::Insert {
                    // TODO: add insert
                } else {
                    y = y.saturating_sub(1);
                }
            },
            Key::Down => {
                if y < height {
                    y = y.saturating_add(1);
                }
            },
            Key::Char('j') => {
                if self.mode == Mode::Insert {
                    // TODO: add insert
                } else if y < height {
                    y = y.saturating_add(1);
                }
            },
            Key::Left => {
                if x > 0 {
                    x -= 1;
                } else if y > 0 {
                    y -= 1;
                    if let Some(row) = self.document.row(y) {
                        x = row.len();
                    } else {
                        x = 0;
                    }
                }
            },
            Key::Char('h') => {
                if self.mode == Mode::Insert {
                    // TODO: add insert
                } else if x > 0 {
                    x -= 1;
                } else if y > 0 {
                    y -= 1;
                    if let Some(row) = self.document.row(y) {
                        x = row.len();
                    } else {
                        x = 0;
                    }
                }
            },
            Key::Right => {
                if x < width {
                    x += 1;
                } else if y < height {
                    y += 1;
                    x = 0;
                }
            },
            Key::Char('l') => {
                if self.mode == Mode::Insert {
                    // TODO: add insert
                } else if x < width {
                    x += 1;
                } else if y < height {
                    y += 1;
                    x = 0;
                }
            },
            Key::PageUp | Key::Ctrl('u') => {
                y = if y > terminal_height {
                    y.saturating_sub(terminal_height)
                } else {
                    0
                }
            },
            Key::PageDown | Key::Ctrl('d') => {
                y = if y.saturating_add(terminal_height) < height {
                    y.saturating_add(terminal_height)
                } else {
                    height
                }
            },
            Key::Home => x = 0,
            Key::End => x = width,
            _ => (),
        }

        width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };
        if x > width {
            x = width;
        }

        self.current_position = Position { x, y };
    }

    fn refresh_screen(&self) -> Result<(), std::io::Error> {
        Terminal::cursor_hide();
        Terminal::clear_screen();
        Terminal::cursor_position(&Position { x: 0, y: 0 });
        if self.should_quit {
            Terminal::clear_screen();
            println!("Goodbye.\r");
        } else {
            self.draw_rows();
            self.draw_status_bar();
            self.draw_message_bar();
            Terminal::cursor_position(&Position {
                x: self.current_position.x.saturating_sub(self.offset.x),
                y: self.current_position.y.saturating_sub(self.offset.y),
            });
        }
        Terminal::cursor_show();
        Terminal::flush()
    }

    fn draw_status_bar(&self) {
        let mut status;
        let width = self.terminal.size().width as usize;
        let modified_indicator = if self.document.is_dirty() {
            " (modified)"
        } else {
            ""
        };
        let _mode = format!("{:?}", self.mode);
        let mut file_name = "[No Name]".to_string();
        if let Some(name) = &self.document.file_name {
            file_name = name.clone();
            file_name.truncate(20);
        }
        status = format!(
            "{} - {} lines - {}", file_name, self.document.len(), modified_indicator);
        let line_indicator = format!(
            "{}:{}",
            self.current_position.y.saturating_add(1),
            self.current_position.x.saturating_add(1)
        );

        #[allow(clippy::integer_arithmetic)]
        let len = status.len() + line_indicator.len();
        status.push_str(&" ".repeat(width.saturating_sub(len)));
        status = format!("{}{}", status, line_indicator);
        status.truncate(width);
        Terminal::set_bg_color(STATUS_BG_COLOR);
        Terminal::set_fg_color(STATUS_FG_COLOR);
        println!("{}\r", status);
        Terminal::reset_fg_color();
        Terminal::reset_bg_color();
    }

    fn draw_message_bar(&self) {
        Terminal::clear_current_line();
        let message = &self.status_message;
        if Instant::now() - message.time < Duration::new(5, 0) {
            let mut text = message.text.clone();
            text.truncate(self.terminal.size().width as usize);
            print!("{}", text);
        }
    }

    pub fn draw_row(&self, row: &Row) {
        let width = self.terminal.size().width as usize;
        let start = self.offset.x;
        let end = self.offset.x + width;
        let row = row.render(start, end);
        println!("{}\r", row);
    }

    #[allow(clippy::arithmetic_side_effects, clippy::integer_division)]
    fn draw_rows(&self) {
        let height = self.terminal.size().height;
        for terminal_row in 0..height {
            Terminal::clear_current_line();
            if let Some(row) = self
                .document
                .row(self.offset.y.saturating_add(terminal_row as usize)) {
                self.draw_row(row);
            } else if self.document.is_empty() && terminal_row == height / 3 {
                self.draw_welcome_message();
            } else {
                println!("~\r");
            }
        }
    }
}

fn die(e: std::io::Error) {
    Terminal::clear_screen();
    panic!("{}", e);
}
