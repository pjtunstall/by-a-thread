use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use macroquad::prelude::*;

use crate::lobby::ui::{LobbyUi, UiInputError};
use common::{input::UiKey, player::Color as PlayerColor};

const PROMPT: &str = "> ";
const FONT_SIZE: f32 = 24.0;
const SIDE_PAD: f32 = 20.0;
const BOTTOM_PAD: f32 = 40.0;

const TEXT_COLOR: Color = WHITE;
const WARNING_COLOR: Color = YELLOW;
const ERROR_COLOR: Color = RED;
const PROMPT_COLOR: Color = LIGHTGRAY;
const INPUT_COLOR: Color = LIGHTGRAY;
const BANNER_COLOR: Color = YELLOW;
const BACKGROUND_COLOR: Color = BLACK;
const BANNER_COLUMN_GAP: f32 = 12.0;

fn player_color_to_text_color(color: PlayerColor) -> Color {
    match color {
        PlayerColor::RED => RED,
        PlayerColor::LIME => LIME,
        PlayerColor::PINK => PINK,
        PlayerColor::YELLOW => YELLOW,
        PlayerColor::GREEN => GREEN,
        PlayerColor::BLUE => BLUE,
        PlayerColor::MAROON => MAROON,
        PlayerColor::ORANGE => ORANGE,
        PlayerColor::PURPLE => PURPLE,
        PlayerColor::SKYBLUE => SKYBLUE,
    }
}

#[derive(Debug)]
pub struct Gui {
    pub message_history: Vec<(String, Color)>,
    input_buffer: String,
    max_history_lines: usize,
    cursor_pos: usize,
    right_arrow_last_pressed: Option<Instant>,
    left_arrow_last_pressed: Option<Instant>,
    backspace_last_pressed: Option<Instant>,
    local_player_color: Option<PlayerColor>,
    scroll_offset: usize,
    up_arrow_last_pressed: Option<Instant>,
    down_arrow_last_pressed: Option<Instant>,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            input_buffer: String::new(),
            message_history: Vec::new(),
            max_history_lines: 1024,
            cursor_pos: 0,
            right_arrow_last_pressed: None,
            left_arrow_last_pressed: None,
            backspace_last_pressed: None,
            local_player_color: None,
            scroll_offset: 0,
            up_arrow_last_pressed: None,
            down_arrow_last_pressed: None,
        }
    }

    fn add_history(&mut self, message: &str, color: Color) {
        self.message_history.push((message.to_string(), color));

        if self.message_history.len() > self.max_history_lines {
            self.message_history.remove(0);
        }

        // Reset scroll offset to show newest messages.
        self.scroll_offset = 0;
    }

    pub fn draw(&self, should_show_input: bool, show_cursor: bool, font: Option<&Font>) {
        push_camera_state();
        set_default_camera();

        clear_background(BACKGROUND_COLOR);

        let line_height = FONT_SIZE * 1.2;
        let max_width = screen_width() - 2.0 * SIDE_PAD; // of a line of text.

        // Start at the bottom,
        let mut current_baseline = screen_height() - BOTTOM_PAD;

        if should_show_input {
            self.draw_input(
                &mut current_baseline,
                line_height,
                max_width,
                show_cursor,
                font,
            );
        } // and move the current_baseline to the line above the input.
        self.draw_chat_history(current_baseline, line_height, max_width, font);

        pop_camera_state();
    }

    fn draw_input(
        &self,
        current_baseline: &mut f32,
        line_height: f32,
        max_width: f32,
        show_cursor: bool,
        font: Option<&Font>,
    ) {
        let input_color = self
            .local_player_color
            .map(player_color_to_text_color)
            .unwrap_or(INPUT_COLOR);
        let full_input_text = format!("{}{}", PROMPT, self.input_buffer);
        let input_lines = self.wrap_text(&full_input_text, max_width, font);

        let input_start_y = *current_baseline - ((input_lines.len() as f32 - 1.0) * line_height);
        let mut draw_y = input_start_y;

        for line in input_lines.iter() {
            if let Some(font) = font {
                draw_text_ex(
                    line,
                    SIDE_PAD,
                    draw_y,
                    TextParams {
                        font: Some(font),
                        font_size: FONT_SIZE as u16,
                        color: input_color,
                        ..Default::default()
                    },
                );
            } else {
                draw_text(line, SIDE_PAD, draw_y, FONT_SIZE, input_color);
            }

            draw_y += line_height;
        }

        if show_cursor && (get_time() * 2.0) as i32 % 2 == 0 {
            self.draw_cursor(input_start_y, &input_lines, line_height, font, input_color);
        }

        *current_baseline -= input_lines.len() as f32 * line_height;
    }

    fn draw_cursor(
        &self,
        input_start_y: f32,
        input_lines: &Vec<String>,
        line_height: f32,
        font: Option<&Font>,
        input_color: Color,
    ) {
        // Determine the logical index of the cursor within the FULL text (including the prompt). We use char indices because cursor_pos is a char count.
        let prompt_len = PROMPT.chars().count();
        let target_char_index = self.cursor_pos + prompt_len;

        let mut chars_processed = 0;
        let mut cursor_found = false;
        let mut cursor_x = SIDE_PAD;
        let mut cursor_y = input_start_y;

        for (i, line) in input_lines.iter().enumerate() {
            let line_len = line.chars().count();

            // Check if the cursor sits on this line. We use <= because the
            // cursor can be AT the very end of the line.
            if target_char_index <= chars_processed + line_len {
                // The cursor is on this line.
                let index_in_line = target_char_index - chars_processed;

                // Get the text strictly before the cursor on this specific line.
                let sub_string: String = line.chars().take(index_in_line).collect();

                let text_width = self.measure_text_strict(&sub_string, font);

                cursor_x = SIDE_PAD + text_width;
                cursor_y = input_start_y + (i as f32 * line_height);
                cursor_found = true;
                break;
            }

            chars_processed += line_len;
        }

        if !cursor_found && !input_lines.is_empty() {
            // If the cursor is at the very end of all the text:
            let last_idx = input_lines.len() - 1;
            let last_line = &input_lines[last_idx];
            let text_width = self.measure_text_strict(last_line, font);
            cursor_x = SIDE_PAD + text_width;
            cursor_y = input_start_y + (last_idx as f32 * line_height);
        } else if input_lines.is_empty() {
            // If there's no input text:
            cursor_x = SIDE_PAD + self.measure_text_strict(PROMPT, font);
            cursor_y = input_start_y;
        }

        draw_rectangle(
            cursor_x,
            cursor_y - FONT_SIZE + 5.0,
            2.0,
            FONT_SIZE,
            input_color,
        );
    }

    fn draw_chat_history(
        &self,
        mut current_baseline: f32,
        line_height: f32,
        max_width: f32,
        font: Option<&Font>,
    ) {
        let mut banner_label_width: f32 = 0.0;
        for (message, _) in &self.message_history {
            if let Some((label, _)) = message.split_once('\t') {
                banner_label_width = banner_label_width.max(self.measure_text_strict(label, font));
            }
        }

        let banner_value_x = SIDE_PAD + banner_label_width + BANNER_COLUMN_GAP;
        let banner_value_width = (max_width - banner_label_width - BANNER_COLUMN_GAP).max(0.0);

        for (message, color) in self.message_history.iter().rev().skip(self.scroll_offset) {
            if let Some((label, value)) = message.split_once('\t') {
                let lines = if banner_value_width > 0.0 {
                    self.wrap_text(value, banner_value_width, font)
                } else {
                    self.wrap_text(value, max_width, font)
                };

                for (line_index, line) in lines.iter().enumerate().rev() {
                    if current_baseline < line_height * 2.0 {
                        break;
                    }

                    if line_index == 0 {
                        if let Some(font) = font {
                            draw_text_ex(
                                label,
                                SIDE_PAD,
                                current_baseline,
                                TextParams {
                                    font: Some(font),
                                    font_size: FONT_SIZE as u16,
                                    color: *color,
                                    ..Default::default()
                                },
                            );
                        } else {
                            draw_text(label, SIDE_PAD, current_baseline, FONT_SIZE, *color);
                        }
                    }

                    let value_x = if banner_value_width > 0.0 {
                        banner_value_x
                    } else {
                        SIDE_PAD
                    };
                    if let Some(font) = font {
                        draw_text_ex(
                            line,
                            value_x,
                            current_baseline,
                            TextParams {
                                font: Some(font),
                                font_size: FONT_SIZE as u16,
                                color: *color,
                                ..Default::default()
                            },
                        );
                    } else {
                        draw_text(line, value_x, current_baseline, FONT_SIZE, *color);
                    }
                    current_baseline -= line_height;
                }
                continue;
            }

            let lines = self.wrap_text(message, max_width, font);
            for line in lines.iter().rev() {
                if current_baseline < line_height * 2.0 {
                    break;
                }
                if let Some(font) = font {
                    draw_text_ex(
                        line,
                        SIDE_PAD,
                        current_baseline,
                        TextParams {
                            font: Some(font),
                            font_size: FONT_SIZE as u16,
                            color: *color,
                            ..Default::default()
                        },
                    );
                } else {
                    draw_text(line, SIDE_PAD, current_baseline, FONT_SIZE, *color);
                }
                current_baseline -= line_height;
            }
        }
    }

    // Measure text width, forcing the inclusion of trailing spaces.
    fn measure_text_strict(&self, text: &str, font: Option<&Font>) -> f32 {
        let font_size = FONT_SIZE as u16;
        let line_spacing = 1.0;

        if text.ends_with(' ') {
            // Trick: append a pipe '|', measure, then subtract the pipe's width.
            // This forces the engine to account for the pixels of the trailing space.
            let temp = format!("{}|", text);
            let temp_width = measure_text(&temp, font, font_size, line_spacing).width;
            let pipe_width = measure_text("|", font, font_size, line_spacing).width;
            temp_width - pipe_width
        } else {
            measure_text(text, font, font_size, line_spacing).width
        }
    }

    fn wrap_text(&self, text: &str, max_width: f32, font: Option<&Font>) -> Vec<String> {
        let mut wrapped_lines = Vec::new();

        for line in text.lines() {
            if line.is_empty() {
                wrapped_lines.push(String::new());
                continue;
            }

            // This will become a row of text as it appears on screen.
            let mut current_line = String::new();

            let parts: Vec<&str> = line.split(' ').collect();

            for (i, part) in parts.iter().enumerate() {
                // Define 'word' (the chunk we are trying to fit).
                // If i > 0, this part was preceded by a space, so we include it.
                let word = if i == 0 {
                    part.to_string()
                } else {
                    format!(" {}", part)
                };

                let line_with_word = format!("{}{}", current_line, word);
                let line_with_word_width = self.measure_text_strict(&line_with_word, font);

                // Case 1: Word fits on the current line.
                if line_with_word_width <= max_width {
                    current_line = line_with_word;
                    continue;
                }

                // Word doesn't fit: need to handle wrapping.
                let word_width = self.measure_text_strict(&word, font);
                let is_at_prompt_only = current_line.trim() == ">";
                let word_fits_on_new_line = word_width <= max_width;

                // Case 2: Standard wrap: word fits on a new line and is not
                // the prompt, >, so add it to the current line.
                if word_fits_on_new_line && !is_at_prompt_only {
                    wrapped_lines.push(current_line);
                    current_line = word.to_string();
                }
                // Case 3: Force-split: either word is too wide OR we're at the prompt.
                else {
                    // We enter this case when EITHER:
                    // a) The word is wider than the entire screen width, OR
                    // b) current_line is just the prompt (">") and we want to keep
                    //    the next word attached to it rather than wrapping the word
                    //    to a new line (which would leave ">" stranded alone).

                    // We append characters to whatever is already in
                    // current_line (which might be ">"), so the prompt stays
                    // attached.
                    for character in word.chars() {
                        let line_with_char = format!("{}{}", current_line, character);
                        let line_with_char_width = self.measure_text_strict(&line_with_char, font);

                        if line_with_char_width > max_width {
                            // Current line is now full, so push it and start fresh.
                            wrapped_lines.push(current_line);
                            current_line = character.to_string();
                        } else {
                            // Character fits, so keep building on `current_line`.
                            current_line = line_with_char;
                        }
                    }
                }
            }

            if !current_line.is_empty() {
                wrapped_lines.push(current_line);
            }
        }

        wrapped_lines
    }

    fn delete_previous_char(&mut self) {
        let byte_index = self
            .input_buffer
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_pos)
            .unwrap_or(self.input_buffer.len());

        self.input_buffer.remove(byte_index);
    }
}

impl LobbyUi for Gui {
    fn show_message(&mut self, message: &str) {
        self.add_history(message, TEXT_COLOR);
    }

    fn show_message_with_color(&mut self, message: &str, color: PlayerColor) {
        let text_color = player_color_to_text_color(color);
        self.add_history(message, text_color);
    }

    fn set_local_player_color(&mut self, color: PlayerColor) {
        self.local_player_color = Some(color);
    }

    fn show_error(&mut self, message: &str) {
        self.add_history(
            &format!("[ERROR] {}.", message.trim_end_matches('.')),
            ERROR_COLOR,
        );
    }

    fn show_warning(&mut self, message: &str) {
        if message.contains("Waiting for server") {
            self.add_history(message, WARNING_COLOR);
        } else {
            self.add_history(
                &format!("{}.", message.trim_end_matches('.')),
                WARNING_COLOR,
            );
        }
    }

    fn show_prompt(&mut self, prompt: &str) {
        self.add_history(prompt, PROMPT_COLOR);
    }

    fn print_client_banner(&mut self, protocol_id: u64, server_addr: SocketAddr) {
        self.add_history(&format!("  Game version:\t{}", protocol_id), BANNER_COLOR);
        self.add_history(&format!("  Connecting to:\t{}", server_addr), BANNER_COLOR);
    }

    fn show_banner_message(&mut self, message: &str) {
        self.add_history(&format!("  {}", message), BANNER_COLOR);
    }

    fn draw(&self, should_show_input: bool, show_cursor: bool, font: Option<&Font>) {
        Gui::draw(self, should_show_input, show_cursor, font);
    }

    fn flush_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.right_arrow_last_pressed = None;
        self.left_arrow_last_pressed = None;
        self.backspace_last_pressed = None;
        self.up_arrow_last_pressed = None;
        self.down_arrow_last_pressed = None;
        while get_char_pressed().is_some() {}
    }

    fn poll_input(&mut self, limit: usize, is_host: bool) -> Result<Option<String>, UiInputError> {
        if is_key_pressed(KeyCode::Tab) {
            if is_host {
                self.input_buffer.clear();
                self.cursor_pos = 0;
                return Ok(Some("\t".to_string()));
            } else {
                return Ok(None);
            }
        }

        let char_count = self.input_buffer.chars().count();

        let initial_delay = Duration::from_millis(500);
        let repeat_rate = Duration::from_millis(32);

        if is_key_down(KeyCode::Up) {
            match self.up_arrow_last_pressed {
                Some(last) => {
                    if last.elapsed() >= repeat_rate {
                        if self.scroll_offset < self.message_history.len().saturating_sub(1) {
                            self.scroll_offset += 1;
                        }
                        self.up_arrow_last_pressed = Some(Instant::now());
                    }
                }
                None => {
                    if self.scroll_offset < self.message_history.len().saturating_sub(1) {
                        self.scroll_offset += 1;
                    }
                    self.up_arrow_last_pressed = Some(Instant::now() + initial_delay);
                }
            }
        } else {
            self.up_arrow_last_pressed = None;
        }

        if is_key_down(KeyCode::Down) {
            match self.down_arrow_last_pressed {
                Some(last) => {
                    if last.elapsed() >= repeat_rate {
                        if self.scroll_offset > 0 {
                            self.scroll_offset -= 1;
                        }
                        self.down_arrow_last_pressed = Some(Instant::now());
                    }
                }
                None => {
                    if self.scroll_offset > 0 {
                        self.scroll_offset -= 1;
                    }
                    self.down_arrow_last_pressed = Some(Instant::now() + initial_delay);
                }
            }
        } else {
            self.down_arrow_last_pressed = None;
        }

        if is_key_down(KeyCode::Left) && self.cursor_pos > 0 {
            match self.left_arrow_last_pressed {
                Some(last) => {
                    if last.elapsed() >= repeat_rate {
                        self.cursor_pos -= 1;
                        self.left_arrow_last_pressed = Some(Instant::now());
                    }
                }
                None => {
                    self.cursor_pos -= 1;
                    self.left_arrow_last_pressed = Some(Instant::now() + initial_delay);
                }
            }
        } else {
            self.left_arrow_last_pressed = None;
        }

        if is_key_down(KeyCode::Right) && self.cursor_pos < char_count {
            match self.right_arrow_last_pressed {
                Some(last) => {
                    if last.elapsed() >= repeat_rate {
                        self.cursor_pos += 1;
                        self.right_arrow_last_pressed = Some(Instant::now());
                    }
                }
                None => {
                    self.cursor_pos += 1;
                    self.right_arrow_last_pressed = Some(Instant::now() + initial_delay);
                }
            }
        } else {
            self.right_arrow_last_pressed = None;
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

        if is_key_down(KeyCode::Backspace) && self.cursor_pos > 0 {
            match self.backspace_last_pressed {
                Some(last) => {
                    if last.elapsed() >= repeat_rate {
                        self.cursor_pos -= 1;
                        self.delete_previous_char();
                        self.backspace_last_pressed = Some(Instant::now());
                    }
                }
                None => {
                    self.cursor_pos -= 1;
                    self.delete_previous_char();
                    self.backspace_last_pressed = Some(Instant::now() + initial_delay);
                }
            }
        } else {
            self.backspace_last_pressed = None;
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

    fn draw_countdown(&mut self, countdown_text: &str, font: Option<&Font>) {
        push_camera_state();
        set_default_camera();

        clear_background(BLACK);

        let font_size = 120.0;
        let text_color = WHITE;

        let text_dimensions = measure_text(countdown_text, font, font_size as u16, 1.0);
        let reference_dimensions =
            if countdown_text.len() == 1 && countdown_text.chars().all(|c| c.is_ascii_digit()) {
                Some(measure_text("10", font, font_size as u16, 1.0))
            } else {
                None
            };
        let screen_center_x = screen_width() / 2.0;
        let screen_center_y = screen_height() / 2.0;

        let x_pos = if let Some(reference_dimensions) = reference_dimensions {
            screen_center_x - reference_dimensions.width / 2.0
                + (reference_dimensions.width - text_dimensions.width) / 2.0
        } else {
            screen_center_x - text_dimensions.width / 2.0
        };
        let y_pos = screen_center_y + text_dimensions.height / 2.0;

        if let Some(font) = font {
            draw_text_ex(
                countdown_text,
                x_pos,
                y_pos,
                TextParams {
                    font: Some(font),
                    font_size: font_size as u16,
                    color: text_color,
                    ..Default::default()
                },
            );
        } else {
            draw_text(countdown_text, x_pos, y_pos, font_size, text_color);
        }

        pop_camera_state();
    }
}
