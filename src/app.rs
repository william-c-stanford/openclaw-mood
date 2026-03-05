use crossterm::event::{self, KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use tokio::sync::mpsc;

use crate::cli;
use crate::persist;
use crate::rain::{Rain, widget::RainWidget};
use crate::chat::{ChatState, widget::ChatWidget};
use crate::input::{InputState, widget::InputWidget};
use crate::gateway::{self, ConnectionStatus, GatewayAction, GatewayCommand};
use crate::settings::{SettingsState, widget::SettingsWidget};
use crate::effects::{EffectManager, EffectsWidget};
use crate::mood::{Mood, MoodDirector, MoodUpdate};
use crate::mood_tag;
use std::time::{Duration, Instant};

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Viewing rain, no input box
    Viewing,
    /// Typing a message, input box visible
    Typing,
    /// Settings menu (full screen, rain paused)
    Settings,
    /// Exiting
    Exiting,
}

pub struct App {
    pub mode: AppMode,
    pub rain: Rain<1024>,
    pub settings: cli::Cli,
    pub bg_color: Option<(u8, u8, u8)>,
    pub chat: ChatState,
    pub input: InputState,
    pub connection_status: ConnectionStatus,
    pub gateway_tx: Option<mpsc::Sender<GatewayCommand>>,
    pub gateway_rx: Option<mpsc::Receiver<GatewayAction>>,
    pub settings_state: Option<SettingsState>,
    pub effects: EffectManager,
    pub mood_director: MoodDirector,
    pub term_width: u16,
    pub term_height: u16,
    /// Tracks last applied mood update for throttling
    last_mood_update: Option<Instant>,
}

impl App {
    pub fn new(width: u16, height: u16, settings: cli::Cli) -> Self {
        let rain = Rain::<1024>::new(width as usize, height as usize, &settings);
        let bg_color = settings.rain_bg_color();

        // Connect gateway if not offline and URL is available
        let (gateway_tx, gateway_rx, status) = if !settings.offline {
            if let Some(config) = gateway::config::GatewayConfig::resolve(
                settings.gateway_url.as_deref(),
                settings.token.as_deref(),
            ) {
                let (tx, rx) = gateway::spawn_gateway(config);
                (Some(tx), Some(rx), ConnectionStatus::Connecting)
            } else {
                (None, None, ConnectionStatus::Disconnected)
            }
        } else {
            (None, None, ConnectionStatus::Disconnected)
        };

        let (br, bg_c, bb) = settings.rain_color();
        let (hr, hg, hb) = settings.head_color();
        let mood_director = MoodDirector::new([br, bg_c, bb], [hr, hg, hb]);

        Self {
            mode: AppMode::Viewing,
            rain,
            settings,
            bg_color,
            chat: ChatState::new(),
            input: InputState::new(),
            connection_status: status,
            gateway_tx,
            gateway_rx,
            settings_state: None,
            effects: EffectManager::new(),
            mood_director,
            term_width: width,
            term_height: height,
            last_mood_update: None,
        }
    }

    /// Returns the minimum interval between mood updates based on user preference.
    fn mood_throttle_interval(&self) -> Option<Duration> {
        match self.settings.mood_frequency.as_deref() {
            Some("off") => None, // mood disabled
            Some("rare") => Some(Duration::from_secs(30)),
            Some("normal") | None => Some(Duration::from_secs(8)),
            Some("expressive") => Some(Duration::from_secs(2)),
            _ => Some(Duration::from_secs(8)),
        }
    }

    /// Try to apply a mood update, respecting the throttle setting.
    /// Returns true if the update was applied.
    fn try_apply_mood(&mut self, update: &MoodUpdate) -> bool {
        let Some(min_interval) = self.mood_throttle_interval() else {
            return false; // mood is off
        };

        if let Some(last) = self.last_mood_update {
            if last.elapsed() < min_interval {
                return false; // throttled
            }
        }

        self.mood_director.apply_mood(update);
        self.last_mood_update = Some(Instant::now());
        true
    }

    pub fn rebuild_rain(&mut self, width: u16, height: u16) {
        self.term_width = width;
        self.term_height = height;
        self.rain = Rain::<1024>::new(width as usize, height as usize, &self.settings);
    }

    pub fn tick(&mut self) {
        if self.mode != AppMode::Settings {
            // Tick mood transitions
            self.mood_director.tick();

            // Apply mood override colors to rain (used on column reset)
            if self.mood_director.is_transitioning() || self.mood_director.current_mood.is_some() {
                let body = self.mood_director.body_color();
                let head = self.mood_director.head_color();
                self.rain.set_override_colors(Some(body), Some(head));
            }

            // Sync emoji pool and density to rain (assigned per-column on reset)
            if self.mood_director.emoji_accents.has_emojis() {
                let pool: Vec<char> = self.mood_director.emoji_accents.current_pool();
                let density = self.mood_director.emoji_accents.effective_density();
                self.rain.set_emoji_accents(pool, density);
            } else {
                self.rain.clear_emoji_accents();
            }

            self.rain.update();
            self.rain.update_screen_buffer().ok();
        }
        self.effects.tick();
    }

    pub fn handle_key(&mut self, key: event::KeyEvent) {
        match self.mode {
            AppMode::Viewing => self.handle_viewing_key(key),
            AppMode::Typing => self.handle_typing_key(key),
            AppMode::Settings => self.handle_settings_key(key),
            AppMode::Exiting => {}
        }
    }

    fn handle_viewing_key(&mut self, key: event::KeyEvent) {
        match key {
            event::KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. }
            | event::KeyEvent { code: KeyCode::Esc, .. }
            | event::KeyEvent { code: KeyCode::Char('q' | 'Q'), modifiers: KeyModifiers::NONE, .. } => {
                self.mode = AppMode::Exiting;
            }
            event::KeyEvent { code: KeyCode::Char('i' | '/'), .. } => {
                self.mode = AppMode::Typing;
            }
            event::KeyEvent { code: KeyCode::Char('s'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.settings_state = Some(SettingsState::from_cli(&self.settings));
                self.mode = AppMode::Settings;
            }
            event::KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::NONE, .. } => {
                // Debug: cycle through moods
                const MOODS: [Option<Mood>; 9] = [
                    None, // neutral/reset
                    Some(Mood::Curious),
                    Some(Mood::Excited),
                    Some(Mood::Contemplative),
                    Some(Mood::Frustrated),
                    Some(Mood::Amused),
                    Some(Mood::Focused),
                    Some(Mood::Serene),
                    None, // back to neutral
                ];
                let current_idx = MOODS.iter().position(|m| *m == self.mood_director.current_mood).unwrap_or(0);
                let next_idx = (current_idx + 1) % MOODS.len();
                let next_mood = MOODS[next_idx];
                let intensity = if next_mood.is_some() { 1.0 } else { 0.0 };
                self.mood_director.apply_mood(&MoodUpdate {
                    mood: next_mood,
                    intensity,
                    custom: None,
                    transition_ms: Some(2500),
                });
            }
            event::KeyEvent { code: KeyCode::Up, .. } => {
                self.chat.scroll_up(3);
            }
            event::KeyEvent { code: KeyCode::Down, .. } => {
                self.chat.scroll_down(3);
            }
            _ => {}
        }
    }

    fn handle_typing_key(&mut self, key: event::KeyEvent) {
        match key {
            event::KeyEvent { code: KeyCode::Esc, .. } => {
                self.mode = AppMode::Viewing;
            }
            event::KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.mode = AppMode::Exiting;
            }
            event::KeyEvent { code: KeyCode::Enter, .. } => {
                if !self.input.is_empty() {
                    let text = self.input.take_text();
                    self.chat.push_user_message(text.clone());
                    // Trigger visual effect at center of screen
                    self.effects.trigger(self.term_width / 2, self.term_height / 2);
                    // Send to gateway if connected
                    if let Some(ref tx) = self.gateway_tx {
                        let _ = tx.try_send(GatewayCommand::SendMessage(text));
                        self.chat.start_streaming();
                    }
                }
            }
            event::KeyEvent { code: KeyCode::Backspace, .. } => {
                self.input.backspace();
            }
            event::KeyEvent { code: KeyCode::Delete, .. } => {
                self.input.delete();
            }
            event::KeyEvent { code: KeyCode::Left, .. } => {
                self.input.move_left();
            }
            event::KeyEvent { code: KeyCode::Right, .. } => {
                self.input.move_right();
            }
            event::KeyEvent { code: KeyCode::Home, .. } => {
                self.input.move_home();
            }
            event::KeyEvent { code: KeyCode::End, .. } => {
                self.input.move_end();
            }
            event::KeyEvent { code: KeyCode::Up, .. } => {
                self.chat.scroll_up(3);
            }
            event::KeyEvent { code: KeyCode::Down, .. } => {
                self.chat.scroll_down(3);
            }
            event::KeyEvent { code: KeyCode::Char(c), modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT, .. } => {
                self.input.insert_char(c);
            }
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key: event::KeyEvent) {
        match key {
            event::KeyEvent { code: KeyCode::Esc, .. }
            | event::KeyEvent { code: KeyCode::Char('s'), modifiers: KeyModifiers::CONTROL, .. } => {
                // Apply settings, persist, and rebuild rain from scratch
                if let Some(ref state) = self.settings_state {
                    self.settings = state.apply_to_cli(&self.settings);
                    self.bg_color = self.settings.rain_bg_color();
                    persist::save(&self.settings);
                }
                self.settings_state = None;
                // Recreate rain entirely with new settings
                self.rain = Rain::<1024>::new(
                    self.term_width as usize,
                    self.term_height as usize,
                    &self.settings,
                );
                // Update mood baseline with new user settings
                let (br, bg_c, bb) = self.settings.rain_color();
                let (hr, hg, hb) = self.settings.head_color();
                self.mood_director.update_base([br, bg_c, bb], [hr, hg, hb]);
                self.mode = AppMode::Viewing;
            }
            event::KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. } => {
                self.settings_state = None;
                self.mode = AppMode::Exiting;
            }
            event::KeyEvent { code: KeyCode::Up, .. } => {
                if let Some(ref mut state) = self.settings_state {
                    state.move_up();
                }
            }
            event::KeyEvent { code: KeyCode::Down, .. } => {
                if let Some(ref mut state) = self.settings_state {
                    state.move_down();
                }
            }
            event::KeyEvent { code: KeyCode::Right, .. } => {
                if let Some(ref mut state) = self.settings_state {
                    state.cycle_next();
                }
            }
            event::KeyEvent { code: KeyCode::Left, .. } => {
                if let Some(ref mut state) = self.settings_state {
                    state.cycle_prev();
                }
            }
            _ => {}
        }
    }

    /// Process pending gateway actions (non-blocking)
    pub fn process_gateway_actions(&mut self) {
        // Drain all pending actions first to avoid borrow conflicts
        let actions: Vec<GatewayAction> = {
            let Some(ref mut rx) = self.gateway_rx else {
                return;
            };
            let mut actions = Vec::new();
            while let Ok(action) = rx.try_recv() {
                actions.push(action);
            }
            actions
        };

        for action in actions {
            match action {
                GatewayAction::Connected => {
                    self.connection_status = ConnectionStatus::Connected;
                }
                GatewayAction::Disconnected(_reason) => {
                    self.connection_status = ConnectionStatus::Connecting; // will auto-reconnect
                }
                GatewayAction::ChatDelta(delta) => {
                    self.chat.append_streaming(&delta);
                    // Scan accumulated streaming text for complete <mood> tags
                    if let Some(ref mut streaming) = self.chat.streaming {
                        let (cleaned, updates) = mood_tag::extract_mood_tags(streaming);
                        if !updates.is_empty() {
                            *streaming = cleaned;
                            for update in &updates {
                                self.try_apply_mood(update);
                            }
                        }
                    }
                }
                GatewayAction::ChatComplete(content) => {
                    // Final extraction pass before finishing
                    if let Some(ref mut streaming) = self.chat.streaming {
                        let (cleaned, updates) = mood_tag::extract_mood_tags(streaming);
                        if !updates.is_empty() {
                            *streaming = cleaned;
                            for update in &updates {
                                self.try_apply_mood(update);
                            }
                        }
                    }
                    self.chat.finish_streaming();
                    if !content.is_empty() {
                        // Complete content provided; if streaming was already finished,
                        // push as a new message
                        if self.chat.streaming.is_none() && !content.is_empty() {
                            let (cleaned, updates) = mood_tag::extract_mood_tags(&content);
                            for update in &updates {
                                self.try_apply_mood(update);
                            }
                            self.chat.push_assistant_message(cleaned);
                        }
                    }
                }
                GatewayAction::Error(msg) => {
                    self.chat.finish_streaming();
                    self.chat.push_assistant_message(format!("[error] {msg}"));
                }
                GatewayAction::MoodUpdate(update) => {
                    self.try_apply_mood(&update);
                }
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        match self.mode {
            AppMode::Settings => {
                if let Some(ref state) = self.settings_state {
                    frame.render_widget(SettingsWidget::new(state), area);
                }
            }
            _ => {
                // Layer 1: Rain background
                frame.render_stateful_widget(
                    RainWidget::new().bg(self.bg_color),
                    area,
                    &mut self.rain,
                );

                // Layer 2: Chat messages (only if there are messages)
                if !self.chat.messages.is_empty() || self.chat.streaming.is_some() {
                    let chat_area = ChatWidget::chat_area(area);
                    frame.render_widget(ChatWidget::new(&self.chat), chat_area);
                }

                // Layer 3: Effects overlay
                if self.effects.has_active() {
                    frame.render_widget(EffectsWidget::new(&mut self.effects), area);
                }

                // Layer 4: Input box (visible in Typing mode)
                if self.mode == AppMode::Typing {
                    let input_area = InputWidget::input_area(area);
                    frame.render_widget(InputWidget::new(&self.input, true), input_area);
                }

                // Layer 5: Status bar
                self.draw_status_bar(frame, area);
            }
        }
    }

    fn draw_status_bar(&self, frame: &mut Frame, area: Rect) {
        if area.height < 2 {
            return;
        }
        let status_area = Rect::new(area.x, area.y + area.height - 1, area.width, 1);

        let mode_str = match self.mode {
            AppMode::Viewing => "VIEWING",
            AppMode::Typing => "TYPING",
            _ => "",
        };

        let hint = match self.mode {
            AppMode::Viewing => "i:chat  m:mood  Ctrl+S:settings  q:quit",
            AppMode::Typing => "Esc:back  Enter:send",
            _ => "",
        };

        let conn_str = match self.connection_status {
            ConnectionStatus::Connected => " CONNECTED ",
            ConnectionStatus::Connecting => " CONNECTING ",
            ConnectionStatus::Disconnected => " OFFLINE ",
        };
        let conn_color = match self.connection_status {
            ConnectionStatus::Connected => Color::Green,
            ConnectionStatus::Connecting => Color::Yellow,
            ConnectionStatus::Disconnected => Color::DarkGray,
        };

        let mut spans = vec![
            Span::styled(
                format!(" {mode_str} "),
                Style::default().fg(Color::Black).bg(Color::Green),
            ),
            Span::raw(" "),
        ];

        // Mood indicator (if active)
        if let Some(mood) = self.mood_director.current_mood {
            let [r, g, b] = self.mood_director.body_color();
            let mood_str = format!(" {:?} ", mood).to_uppercase();
            spans.push(Span::styled(
                mood_str,
                Style::default().fg(Color::Black).bg(Color::Rgb(r, g, b)),
            ));
            spans.push(Span::raw(" "));
        }

        spans.push(Span::styled(conn_str, Style::default().fg(Color::Black).bg(conn_color)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));

        let status = Line::from(spans);

        frame.render_widget(Paragraph::new(status), status_area);
    }
}
