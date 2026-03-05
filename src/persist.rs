use crate::cli::{self, Grouping};
use crate::rain::Direction;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Fields that survive across restarts.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Saved {
    pub color: Option<String>,
    pub head: Option<String>,
    pub group: Option<String>,
    pub direction: Option<Direction>,
    pub speed: Option<String>,
    pub shade: Option<bool>,
    pub shade_gradient: Option<String>,
    pub bg_color: Option<String>,
    pub mood_frequency: Option<String>,
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("openclaw-matrix").join("settings.toml"))
}

/// Load persisted settings from disk (returns default if missing/corrupt).
pub fn load() -> Saved {
    let Some(path) = config_path() else {
        return Saved::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Saved::default();
    };
    toml::from_str(&content).unwrap_or_default()
}

/// Save current CLI state to disk.
pub fn save(settings: &cli::Cli) {
    let Some(path) = config_path() else {
        return;
    };
    let group_name = match &settings.group {
        Grouping::EzEmoji(g) => format!("{:?}", g.name).to_lowercase(),
        Grouping::Custom(_) => "custom".to_string(),
    };
    let saved = Saved {
        color: Some(settings.color.clone()),
        head: Some(settings.head.clone()),
        group: Some(group_name),
        direction: Some(settings.direction),
        speed: Some(settings.speed.clone()),
        shade: Some(settings.shade),
        shade_gradient: Some(settings.shade_gradient.clone()),
        bg_color: settings.bg_color.clone(),
        mood_frequency: settings.mood_frequency.clone(),
    };
    let Ok(content) = toml::to_string_pretty(&saved) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, content);
}

/// Apply saved settings onto a Cli struct (saved values override CLI defaults,
/// but explicit CLI args still take priority via clap's own precedence).
pub fn apply(saved: &Saved, settings: &mut cli::Cli) {
    if let Some(ref color) = saved.color {
        settings.color = color.clone();
    }
    if let Some(ref head) = saved.head {
        settings.head = head.clone();
    }
    if let Some(ref group) = saved.group {
        if let Ok(g) = group.parse::<Grouping>() {
            settings.group = g;
        }
    }
    if let Some(direction) = saved.direction {
        settings.direction = direction;
    }
    if let Some(ref speed) = saved.speed {
        settings.speed = speed.clone();
    }
    if let Some(shade) = saved.shade {
        settings.shade = shade;
    }
    if let Some(ref gradient) = saved.shade_gradient {
        settings.shade_gradient = gradient.clone();
    }
    if saved.bg_color.is_some() {
        settings.bg_color = saved.bg_color.clone();
    }
    if saved.mood_frequency.is_some() {
        settings.mood_frequency = saved.mood_frequency.clone();
    }
}
