// Copyright 2026 No Despondency Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use clap::Parser;
use crossterm::{
    cursor::{self, MoveTo, MoveToColumn},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};
#[cfg(any(demo, demoprt))]
use demoexe::DemoContextT as SqlContextT;
use sqlcontrols;
#[cfg(prod)]
use sqlexe::SqlContextT;
use sqlexet::SqlExeTrait;

pub mod help;
use self::help::{classify_help_input, render_help, HelpAction, HelpTopic};

use std::io::{self, Result, Write};

use tracing::debug;
use tracing_subscriber;

/// SQL Command Line Interface
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    // Database to connect to
    #[arg(short, long)]
    database: Option<String>,
}

pub struct CommandPrompt {
    history: Vec<String>,
    current_line: String,
    statement_lines: Vec<String>, // Accumulate multi-line statement
    cursor_pos: usize,
    history_index: Option<usize>,
    line_number: usize, // Track current line in multi-line statement
    help_topic: Option<HelpTopic>,
}

impl CommandPrompt {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            current_line: String::new(),
            statement_lines: Vec::new(),
            cursor_pos: 0,
            history_index: None,
            line_number: 1,
            help_topic: None,
        }
    }

    pub fn run(&mut self, ctxt: &mut impl SqlExeTrait) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), terminal::Clear(ClearType::All), MoveTo(0, 0))?;

        loop {
            self.display_prompt()?;

            match event::read()? {
                Event::Key(KeyEvent {
                    code, modifiers, ..
                }) => match code {
                    KeyCode::Enter => {
                        self.handle_enter(ctxt)?;
                    }
                    KeyCode::Backspace => {
                        self.handle_backspace()?;
                    }
                    KeyCode::Delete => {
                        self.handle_delete()?;
                    }
                    KeyCode::Left => {
                        self.handle_left_arrow()?;
                    }
                    KeyCode::Right => {
                        self.handle_right_arrow()?;
                    }
                    KeyCode::Up => {
                        self.handle_up_arrow()?;
                    }
                    KeyCode::Down => {
                        self.handle_down_arrow()?;
                    }
                    KeyCode::Char(c) => {
                        if modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                            // Cancel current multi-line statement
                            if !self.statement_lines.is_empty()
                                || !self.current_line.trim().is_empty()
                            {
                                self.cancel_statement()?;
                            } else {
                                break;
                            }
                        } else {
                            self.handle_char(c)?;
                        }
                    }
                    KeyCode::Home => {
                        self.cursor_pos = 0;
                    }
                    KeyCode::End => {
                        self.cursor_pos = self.current_line.len();
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        terminal::disable_raw_mode()?;
        execute!(io::stdout(), Print("\nExiting...\n"))?;
        Ok(())
    }

    fn display_prompt(&self) -> Result<()> {
        let mut stdout = io::stdout();

        // Clear the current line
        queue!(
            stdout,
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine)
        )?;

        // Print prompt prefix - different for first line vs continuation
        if self.statement_lines.is_empty() {
            queue!(
                stdout,
                SetForegroundColor(Color::Green),
                Print("sql> "),
                ResetColor
            )?;
        } else {
            queue!(
                stdout,
                SetForegroundColor(Color::Yellow),
                Print(format!("{:3}> ", self.line_number)),
                ResetColor
            )?;
        }

        // Print current line
        queue!(stdout, Print(&self.current_line))?;

        // Position cursor
        let prompt_len = if self.statement_lines.is_empty() {
            5
        } else {
            5
        }; // "sql> " or "  2> "
        queue!(
            stdout,
            MoveTo((prompt_len + self.cursor_pos) as u16, cursor::position()?.1)
        )?;

        stdout.flush()?;
        Ok(())
    }

    fn handle_enter(&mut self, ctxt: &mut impl SqlExeTrait) -> Result<()> {
        let mut stdout = io::stdout();

        // Move to next line
        execute!(stdout, Print("\n"))?;

        let trimmed_line = self.current_line.trim();

        match classify_help_input(trimmed_line, self.help_topic) {
            HelpAction::Show(topic) => {
                let history_entry = trimmed_line.to_string();
                if !history_entry.is_empty()
                    && history_entry.parse::<usize>().is_err()
                    && (self.history.is_empty() || self.history.last() != Some(&history_entry))
                {
                    self.history.push(history_entry);
                }
                self.show_help(topic)?;
                self.help_topic = Some(topic);
                self.reset_statement();
                return Ok(());
            }
            HelpAction::InvalidSelection => {
                terminal::disable_raw_mode()?;
                execute!(
                    stdout,
                    SetForegroundColor(Color::Red),
                    MoveToColumn(0),
                    Print("Unknown help selection. Type help to show the menu.\n"),
                    ResetColor
                )?;
                terminal::enable_raw_mode()?;
                self.current_line.clear();
                self.cursor_pos = 0;
                self.history_index = None;
                return Ok(());
            }
            HelpAction::NotHelp => {}
        }

        if !trimmed_line.is_empty() {
            self.help_topic = None;
        }

        // Handle empty line behavior
        if trimmed_line.is_empty() {
            if self.statement_lines.is_empty() {
                // Not in multiline statement - just show new prompt
                self.current_line.clear();
                self.cursor_pos = 0;
                self.history_index = None;
                return Ok(());
            }
            // In multiline statement - treat as empty line continuation
        }

        // Add current line to statement
        self.statement_lines.push(self.current_line.clone());

        // Check if statement ends with semicolon
        if trimmed_line.ends_with(';') {
            // Execute the complete statement
            let mut complete_statement = self.statement_lines.join(" ");

            // Strip the ending semicolon
            if complete_statement.trim().ends_with(';') {
                complete_statement = complete_statement.trim_end_matches(';').trim().to_string();
            }

            if !complete_statement.trim().is_empty() {
                // Add to history if it's not a duplicate of the last command
                let history_entry = format!("{};", complete_statement);
                if self.history.is_empty() || self.history.last() != Some(&history_entry) {
                    self.history.push(history_entry);
                }

                // Process the command
                terminal::disable_raw_mode()?;
                execute!(
                    stdout,
                    SetForegroundColor(Color::Blue),
                    MoveToColumn(0),
                    Print(format!("Executing: {}\n", complete_statement)), // No semicolon displayed
                    ResetColor
                )?;

                let res = sqlcontrols::stmt_exec(ctxt, &complete_statement);
                match res {
                    Ok(rows) => {
                        for r in rows {
                            execute!(
                                stdout,
                                SetForegroundColor(Color::White),
                                MoveToColumn(0),
                                Print(format!("{}\n", r))
                            )?;
                        }
                        let count_msg = ctxt.print_count();
                        if !count_msg.is_empty() {
                            execute!(
                                stdout,
                                SetForegroundColor(Color::White),
                                MoveToColumn(0),
                                Print(format!("{}\n", count_msg)),
                                ResetColor
                            )?;
                        }
                    }
                    Err(e) => {
                        execute!(
                            stdout,
                            SetForegroundColor(Color::Red),
                            MoveToColumn(0),
                            Print(format!("Error: {:?}\n", e)),
                            ResetColor
                        )?;
                    }
                }
                terminal::enable_raw_mode()?;
            }
            // Reset for next statement
            self.reset_statement();
        } else {
            // Continue to next line of the same statement
            self.line_number += 1;
            self.current_line.clear();
            self.cursor_pos = 0;
            self.history_index = None;
        }

        Ok(())
    }

    fn show_help(&self, topic: HelpTopic) -> Result<()> {
        let mut stdout = io::stdout();
        terminal::disable_raw_mode()?;
        execute!(
            stdout,
            SetForegroundColor(Color::Cyan),
            MoveToColumn(0),
            Print(format!("{}\n", render_help(topic))),
            ResetColor
        )?;
        terminal::enable_raw_mode()?;
        Ok(())
    }

    fn cancel_statement(&mut self) -> Result<()> {
        let mut stdout = io::stdout();
        execute!(
            stdout,
            Print("\n"),
            SetForegroundColor(Color::Red),
            MoveToColumn(0),
            Print("Statement cancelled.\n"),
            ResetColor
        )?;
        self.reset_statement();
        Ok(())
    }

    fn reset_statement(&mut self) {
        self.statement_lines.clear();
        self.current_line.clear();
        self.cursor_pos = 0;
        self.history_index = None;
        self.line_number = 1;
    }

    fn handle_backspace(&mut self) -> Result<()> {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.current_line.remove(self.cursor_pos);
            self.history_index = None;
        } else if !self.statement_lines.is_empty() {
            // If at the beginning of a continuation line, merge with previous line
            if let Some(prev_line) = self.statement_lines.pop() {
                self.cursor_pos = prev_line.len();
                self.current_line = prev_line + &self.current_line;
                if self.line_number > 1 {
                    self.line_number -= 1;
                }
            }
        }
        Ok(())
    }

    fn handle_delete(&mut self) -> Result<()> {
        if self.cursor_pos < self.current_line.len() {
            self.current_line.remove(self.cursor_pos);
            self.history_index = None;
        }
        Ok(())
    }

    fn handle_left_arrow(&mut self) -> Result<()> {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
        Ok(())
    }

    fn handle_right_arrow(&mut self) -> Result<()> {
        if self.cursor_pos < self.current_line.len() {
            self.cursor_pos += 1;
        }
        Ok(())
    }

    fn handle_up_arrow(&mut self) -> Result<()> {
        if self.history.is_empty() {
            return Ok(());
        }

        // Only allow history navigation when not in a multi-line statement
        if !self.statement_lines.is_empty() {
            return Ok(());
        }

        let new_index = match self.history_index {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };

        self.history_index = Some(new_index);
        self.current_line = self.history[new_index].clone();
        self.cursor_pos = self.current_line.len();

        Ok(())
    }

    fn handle_down_arrow(&mut self) -> Result<()> {
        // Only allow history navigation when not in a multi-line statement
        if !self.statement_lines.is_empty() {
            return Ok(());
        }

        match self.history_index {
            None => {}
            Some(i) if i >= self.history.len() - 1 => {
                self.history_index = None;
                self.current_line.clear();
                self.cursor_pos = 0;
            }
            Some(i) => {
                let new_index = i + 1;
                self.history_index = Some(new_index);
                self.current_line = self.history[new_index].clone();
                self.cursor_pos = self.current_line.len();
            }
        }
        Ok(())
    }

    fn handle_char(&mut self, c: char) -> Result<()> {
        self.current_line.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
        self.history_index = None;
        Ok(())
    }
}

fn main() -> Result<()> {
    println!("SQL Command Line Interface");
    println!("Features:");
    println!("- Multi-line statements supported - end with ';'");
    println!("- Type help for command help");
    println!("- Use Ctrl+C to cancel current statement or exit");
    println!("- Use Up/Down arrows for history (when not in multi-line mode)");
    println!("- Use Home/End to jump to line start/end");
    println!();

    tracing_subscriber::fmt::init();
    debug!("The subscriber is now installed and working!");

    // check if cfg demo
    #[cfg(demo)]
    {
        println!("Running in demo mode",);
    }

    let mut ctxt = SqlContextT::new();
    #[cfg(prod)]
    {
        let args = Args::parse();
        let db = args.database.clone();
        if db.is_none() {
            println!("Database not specified");
            return Ok(());
        }
        let database = db.unwrap();
        let sts = ctxt.connect_database(database.as_str());
        if sts != sqlexet::STS_SUCCESS {
            println!("Failed to connect to database, status code: {}", sts);
            return Ok(());
        }
        println!("Connected to database successfully.");
    }

    let mut prompt = CommandPrompt::new();
    let res = prompt.run(&mut ctxt);
    debug!("Prompt result: {:?}", res);

    ctxt.disconnect_database();
    println!("Disconnected from database.");
    Ok(())
}
