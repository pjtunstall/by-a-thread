use std::net::SocketAddr;

use macroquad::prelude::*;

use crate::ui::{ClientUi, UiInputError};
use shared::input::UiKey;

const FONT_SIZE: f32 = 24.0;
const TEXT_COLOR: Color = WHITE;
const ERROR_COLOR: Color = RED;
const PROMPT_COLOR: Color = LIGHTGRAY;
const BANNER_COLOR: Color = YELLOW;
const BACKGROUND_COLOR: Color = BLACK;

pub struct MacroquadUi {
    input_buffer: String,
    pub message_history: Vec<(String, Color)>,
    status_message: String,
    max_history_lines: usize,
    cursor_pos: usize,
}

impl MacroquadUi {
    pub fn new() -> Self {
        Self {
            input_buffer: String::new(),
            message_history: Vec::new(),
            status_message: String::new(),
            max_history_lines: 20,
            cursor_pos: 0,
        }
    }

    fn add_history(&mut self, message: &str, color: Color) {
        self.message_history.push((message.to_string(), color));

        if self.message_history.len() > self.max_history_lines {
            self.message_history.remove(0);
        }
    }

    pub fn draw(&self, show_input: bool) {
        clear_background(BACKGROUND_COLOR);

        let line_height = FONT_SIZE * 1.2;
        let max_width = screen_width() - 20.0;
        let bottom_pad = 10.0;

        // --- 1. WRAP FULL INPUT TEXT ---
        // We do this ONCE. This layout is the source of truth.
        let full_input_text = format!("> {}", self.input_buffer);
        let input_lines = if show_input {
            self.wrap_text(&full_input_text, max_width)
        } else {
            Vec::new()
        };

        // --- 2. CALCULATE LAYOUT ANCHORS ---
        let mut current_y = screen_height() - bottom_pad;
        draw_text(&self.status_message, 10.0, 30.0, FONT_SIZE, ERROR_COLOR);

        // --- 3. DRAW INPUT & FIND CURSOR ---
        if show_input {
            let input_start_y = current_y - ((input_lines.len() as f32 - 1.0) * line_height);
            let mut draw_y = input_start_y;

            for line in input_lines.iter() {
                draw_text(line, 10.0, draw_y, FONT_SIZE, PROMPT_COLOR);
                draw_y += line_height;
            }

            // --- CURSOR CALCULATION ---
            if (get_time() * 2.0) as i32 % 2 == 0 {
                // Determine the logical index of the cursor within the FULL text (including "> ")
                // We use CHAR indices because cursor_pos is a char count.
                let prompt_len = 2; // Length of "> "
                let target_char_index = self.cursor_pos + prompt_len;

                let mut chars_processed = 0;
                let mut cursor_found = false;
                let mut cursor_x = 10.0;
                let mut cursor_y = input_start_y;

                for (i, line) in input_lines.iter().enumerate() {
                    let line_len = line.chars().count();

                    // Check if the cursor sits on this line
                    // We use <= because the cursor can be AT the very end of the line
                    if target_char_index <= chars_processed + line_len {
                        // The cursor is on this line!
                        let index_in_line = target_char_index - chars_processed;

                        // Get the text strictly BEFORE the cursor on this specific line
                        let sub_string: String = line.chars().take(index_in_line).collect();

                        // Measure it to find X
                        let text_width = self.measure_text_strict(&sub_string);

                        cursor_x = 10.0 + text_width;
                        cursor_y = input_start_y + (i as f32 * line_height);
                        cursor_found = true;
                        break;
                    }

                    chars_processed += line_len;
                }

                // Fallback: If cursor is at the very end of the entire text (and loop finished)
                if !cursor_found && !input_lines.is_empty() {
                    let last_idx = input_lines.len() - 1;
                    let last_line = &input_lines[last_idx];
                    let text_width = self.measure_text_strict(last_line);
                    cursor_x = 10.0 + text_width;
                    cursor_y = input_start_y + (last_idx as f32 * line_height);
                } else if input_lines.is_empty() {
                    cursor_x = 10.0 + self.measure_text_strict("> ");
                    cursor_y = input_start_y;
                }

                draw_rectangle(cursor_x, cursor_y - FONT_SIZE + 5.0, 2.0, FONT_SIZE, WHITE);
            }

            current_y -= input_lines.len() as f32 * line_height;
        }

        // --- 4. DRAW HISTORY ---
        for (message, color) in self.message_history.iter().rev() {
            let lines = self.wrap_text(message, max_width);
            for line in lines.iter().rev() {
                if current_y < line_height * 2.0 {
                    break;
                }
                draw_text(line, 10.0, current_y, FONT_SIZE, *color);
                current_y -= line_height;
            }
        }
    }

    /// Measures text width, forcing the inclusion of trailing spaces.
    fn measure_text_strict(&self, text: &str) -> f32 {
        // We use the constants defined at the top of the file.
        // If these ever need to be dynamic, you would pass them as arguments instead.
        let font: Option<&Font> = None;
        let font_size = FONT_SIZE as u16;
        let line_spacing = 1.0;

        if text.ends_with(' ') {
            // Trick: Append a pipe '|', measure, then subtract the pipe's width.
            // This forces the engine to account for the pixels of the trailing space.
            let temp = format!("{}|", text);
            let temp_width = measure_text(&temp, font, font_size, line_spacing).width;
            let pipe_width = measure_text("|", font, font_size, line_spacing).width;
            temp_width - pipe_width
        } else {
            measure_text(text, font, font_size, line_spacing).width
        }
    }

    fn wrap_text(&self, text: &str, max_width: f32) -> Vec<String> {
        let mut wrapped_lines = Vec::new();

        // for line in text.lines() {
        //     if line.is_empty() {
        //         wrapped_lines.push(String::new());
        //         continue;
        //     }

        // This will be
        let mut current_line = String::new();

        let parts: Vec<&str> = text.split(' ').collect();

        for (i, part) in parts.iter().enumerate() {
            // Define 'word' (the chunk we are trying to fit).
            // If i > 0, this part was preceded by a space, so we include it.
            let word = if i == 0 {
                part.to_string()
            } else {
                format!(" {}", part)
            };

            let line_with_word = format!("{}{}", current_line, word);
            let line_with_word_width = self.measure_text_strict(&line_with_word);

            // Case 1: Word fits on the current line.
            if line_with_word_width <= max_width {
                current_line = line_with_word;
                continue;
            }

            // Word doesn't fit - need to handle wrapping.
            let word_width = self.measure_text_strict(&word);
            let is_at_prompt_only = current_line.trim() == ">";
            let word_fits_on_new_line = word_width <= max_width;

            // Case 2: Standard wrap - word fits on a new line and is not the prompt, >, so add it to the current line.
            if word_fits_on_new_line && !is_at_prompt_only {
                wrapped_lines.push(current_line);
                current_line = word.to_string();
            }
            // Case 3: Force-split - either word is too wide OR we're at the prompt.
            else {
                // We enter this case when EITHER:
                // a) The word is wider than the entire screen width, OR
                // b) current_line is just the prompt (">") and we want to keep
                //    the next word attached to it rather than wrapping the word
                //    to a new line (which would leave ">" stranded alone)

                // Important: we append characters to whatever is already in
                // current_line (which might be ">"), so the prompt stays attached
                for character in word.chars() {
                    let line_with_char = format!("{}{}", current_line, character);
                    let line_with_char_width = self.measure_text_strict(&line_with_char);

                    if line_with_char_width > max_width {
                        // Current line is now full - push it and start fresh
                        wrapped_lines.push(current_line);
                        current_line = character.to_string();
                    } else {
                        // Character fits - keep building on current_line
                        current_line = line_with_char;
                    }
                }
            }
        }

        if !current_line.is_empty() {
            wrapped_lines.push(current_line);
        }
        // }

        wrapped_lines
    }
}

impl ClientUi for MacroquadUi {
    fn show_message(&mut self, message: &str) {
        self.add_history(message, TEXT_COLOR);
    }

    fn show_error(&mut self, message: &str) {
        self.add_history(&format!("[ERROR] {}", message), ERROR_COLOR);
    }

    fn show_prompt(&mut self, prompt: &str) {
        self.add_history(prompt, PROMPT_COLOR);
    }

    fn show_status_line(&mut self, message: &str) {
        self.status_message = message.to_string();
    }

    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr, client_id: u64) {
        self.add_history(&format!("  Game version:  {}", protocol_id), BANNER_COLOR);
        self.add_history(&format!("  Connecting to: {}", server_addr), BANNER_COLOR);
        self.add_history(&format!("  Your ID:       {}", client_id), BANNER_COLOR);
    }

    fn poll_input(&mut self, limit: usize) -> Result<Option<String>, UiInputError> {
        if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::C) {
            // TODO: Send a disconnect signal.
            std::process::exit(0);
        }

        let char_count = self.input_buffer.chars().count();

        if is_key_pressed(KeyCode::Left) && self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }

        if is_key_pressed(KeyCode::Right) && self.cursor_pos < char_count {
            self.cursor_pos += 1;
        }

        while let Some(char_code) = get_char_pressed() {
            let c = char_code as char;
            let at_limit = self.input_buffer.len() >= limit;

            if !at_limit && !c.is_control() {
                let byte_index = self
                    .input_buffer
                    .char_indices()
                    .map(|(i, _)| i)
                    .nth(self.cursor_pos)
                    .unwrap_or(self.input_buffer.len());

                self.input_buffer.insert(byte_index, c);
                self.cursor_pos += 1;
            }
        }

        if is_key_pressed(KeyCode::Backspace) && self.cursor_pos > 0 {
            self.cursor_pos -= 1;

            let byte_index = self
                .input_buffer
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.cursor_pos)
                .unwrap_or(self.input_buffer.len());

            self.input_buffer.remove(byte_index);
        }

        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            let line = self.input_buffer.drain(..).collect();
            self.input_buffer.clear();
            self.cursor_pos = 0;
            return Ok(Some(line));
        }

        if is_key_pressed(KeyCode::Escape) {
            self.input_buffer.clear();
            self.cursor_pos = 0;
        }

        Ok(None)
    }

    fn poll_single_key(&mut self) -> Result<Option<UiKey>, UiInputError> {
        if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::C) {
            return Err(UiInputError::Disconnected);
        }

        if let Some(key_code) = get_last_key_pressed() {
            let ui_key = match key_code {
                KeyCode::Enter => Some(UiKey::Enter),
                KeyCode::Backspace => Some(UiKey::Backspace),
                KeyCode::Escape => Some(UiKey::Esc),
                KeyCode::Tab => Some(UiKey::Tab),
                _ => {
                    if let Some(char_code) = get_char_pressed() {
                        Some(UiKey::Char(char_code as char))
                    } else {
                        None
                    }
                }
            };
            return Ok(ui_key);
        }

        Ok(None)
    }

    fn draw_countdown(&mut self, countdown_text: &str) {
        clear_background(BLACK);

        let font_size = 120.0;
        let text_color = WHITE;

        let text_dimensions = measure_text(countdown_text, None, font_size as u16, 1.0);
        let screen_center_x = screen_width() / 2.0;
        let screen_center_y = screen_height() / 2.0;

        let x_pos = screen_center_x - text_dimensions.width / 2.0;
        let y_pos = screen_center_y + text_dimensions.height / 2.0;

        draw_text(countdown_text, x_pos, y_pos, font_size, text_color);
    }
}
