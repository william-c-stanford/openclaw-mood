pub mod widget;

use crate::cli;
use crate::rain::Direction;

/// A single setting entry with its possible values
struct SettingEntry {
    label: &'static str,
    options: Vec<String>,
    selected: usize,
}

impl SettingEntry {
    fn current(&self) -> &str {
        &self.options[self.selected]
    }

    fn cycle_next(&mut self) {
        self.selected = (self.selected + 1) % self.options.len();
    }

    fn cycle_prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.options.len() - 1;
        } else {
            self.selected -= 1;
        }
    }
}

/// Full-screen settings state
pub struct SettingsState {
    entries: Vec<SettingEntry>,
    pub cursor: usize,
}

impl SettingsState {
    /// Build settings entries from current Cli state
    pub fn from_cli(settings: &cli::Cli) -> Self {
        let color_options = vec![
            "green".into(), "red".into(), "blue".into(), "white".into(),
            "0,255,0".into(), "255,0,0".into(), "0,0,255".into(),
            "0,255,255".into(), "255,0,255".into(), "255,255,0".into(),
        ];
        let current_color = find_index(&color_options, &settings.color);

        let head_options = vec![
            "white".into(), "green".into(), "red".into(), "blue".into(),
            "#FFFFFF".into(), "#00FF00".into(), "#FF0000".into(),
        ];
        let current_head = find_index(&head_options, &settings.head);

        let group_options = vec![
            "bin".into(), "jap".into(), "classic".into(), "num".into(),
            "alphalow".into(), "alphaup".into(), "arrow".into(),
            "cards".into(), "clock".into(), "crab".into(),
            "earth".into(), "emojis".into(), "moon".into(),
            "shapes".into(), "smile".into(), "plants".into(),
            "opensource".into(), "pglangs".into(),
        ];
        let group_name = match &settings.group {
            cli::Grouping::EzEmoji(g) => format!("{:?}", g.name).to_lowercase(),
            cli::Grouping::Custom(_) => "custom".into(),
        };
        let current_group = find_index(&group_options, &group_name);

        let direction_options = vec![
            "south".into(), "north".into(), "west".into(), "east".into(),
        ];
        let current_dir = match settings.direction {
            Direction::Down => 0,
            Direction::Up => 1,
            Direction::Left => 2,
            Direction::Right => 3,
        };

        let speed_options = vec![
            "0,200".into(), "0,100".into(), "0,50".into(),
            "50,200".into(), "100,300".into(), "0,400".into(),
        ];
        let current_speed = find_index(&speed_options, &settings.speed);

        let shade_options = vec!["off".into(), "on".into()];
        let current_shade = if settings.shade { 1 } else { 0 };

        let gradient_options = vec![
            "#000000".into(), "#001100".into(), "#110000".into(),
            "#000011".into(), "#111100".into(), "#001111".into(),
        ];
        let current_gradient = find_index(&gradient_options, &settings.shade_gradient);

        let mood_options = vec![
            "off".into(), "rare".into(), "normal".into(), "expressive".into(),
        ];
        let current_mood = match settings.mood_frequency.as_deref() {
            Some("off") => 0,
            Some("rare") => 1,
            Some("normal") | None => 2,
            Some("expressive") => 3,
            _ => 2,
        };

        let entries = vec![
            SettingEntry { label: "Color", options: color_options, selected: current_color },
            SettingEntry { label: "Head", options: head_options, selected: current_head },
            SettingEntry { label: "Group", options: group_options, selected: current_group },
            SettingEntry { label: "Direction", options: direction_options, selected: current_dir },
            SettingEntry { label: "Speed", options: speed_options, selected: current_speed },
            SettingEntry { label: "Shade", options: shade_options, selected: current_shade },
            SettingEntry { label: "Gradient", options: gradient_options, selected: current_gradient },
            SettingEntry { label: "Mood", options: mood_options, selected: current_mood },
        ];

        Self { entries, cursor: 0 }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor < self.entries.len() - 1 {
            self.cursor += 1;
        }
    }

    pub fn cycle_next(&mut self) {
        self.entries[self.cursor].cycle_next();
    }

    pub fn cycle_prev(&mut self) {
        self.entries[self.cursor].cycle_prev();
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn entry_label(&self, i: usize) -> &str {
        self.entries[i].label
    }

    pub fn entry_value(&self, i: usize) -> &str {
        self.entries[i].current()
    }

    /// Apply current settings back to a Cli struct.
    /// Returns a modified clone that can be used to recreate Rain.
    pub fn apply_to_cli(&self, base: &cli::Cli) -> cli::Cli {
        let mut settings = base.clone();

        settings.color = self.entries[0].current().to_string();
        settings.head = self.entries[1].current().to_string();

        // Group: parse from string
        let group_str = self.entries[2].current();
        if let Ok(g) = group_str.parse::<cli::Grouping>() {
            settings.group = g;
        }

        // Direction
        let dir_str = self.entries[3].current();
        if let Ok(d) = dir_str.parse::<Direction>() {
            settings.direction = d;
        }

        settings.speed = self.entries[4].current().to_string();
        settings.shade = self.entries[5].current() == "on";
        settings.shade_gradient = self.entries[6].current().to_string();
        settings.mood_frequency = Some(self.entries[7].current().to_string());

        settings
    }
}

fn find_index(options: &[String], value: &str) -> usize {
    options.iter().position(|o| o == value).unwrap_or(0)
}
