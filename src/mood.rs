use std::time::{Duration, Instant};

/// Predefined mood presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Neutral,
    Curious,
    Excited,
    Contemplative,
    Frustrated,
    Amused,
    Focused,
    Serene,
}

/// Visual parameters derived from a mood
#[derive(Debug, Clone)]
pub struct MoodVisuals {
    pub body_color: [u8; 3],
    pub head_color: [u8; 3],
    pub speed_multiplier: f32,
    /// Emoji accents: scattered among base chars at `emoji_density` rate.
    /// Multiple variants for visual variety. None = no emoji accents.
    pub emojis: Option<Vec<char>>,
    /// Fraction of strands that get emoji head (0.0-1.0, typically 0.10 = 10%)
    pub emoji_density: f32,
}

impl Mood {
    pub fn visuals(&self) -> MoodVisuals {
        match self {
            Mood::Neutral => MoodVisuals {
                body_color: [0, 255, 0],
                head_color: [255, 255, 255],
                speed_multiplier: 1.0,
                emojis: None,
                emoji_density: 0.0,
            },
            Mood::Curious => MoodVisuals {
                body_color: [0, 120, 255],
                head_color: [180, 220, 255],
                speed_multiplier: 1.3,
                emojis: Some(vec!['?', '\u{1F50D}', '\u{1F914}', '\u{1F9D0}', '\u{2753}']),
                emoji_density: 0.08,
            },
            Mood::Excited => MoodVisuals {
                body_color: [255, 50, 200],
                head_color: [255, 255, 0],
                speed_multiplier: 0.6,
                emojis: Some(vec![
                    '\u{2728}', '\u{1F525}', '\u{26A1}', '\u{1F4A5}', '\u{1F389}', '\u{1F680}',
                ]),
                emoji_density: 0.12,
            },
            Mood::Contemplative => MoodVisuals {
                body_color: [60, 0, 180],
                head_color: [140, 100, 255],
                speed_multiplier: 1.5,
                emojis: Some(vec!['\u{1F4AD}', '\u{2728}', '\u{1F30C}', '\u{269B}']),
                emoji_density: 0.06,
            },
            Mood::Frustrated => MoodVisuals {
                body_color: [255, 60, 0],
                head_color: [255, 200, 0],
                speed_multiplier: 0.7,
                emojis: Some(vec![
                    '\u{1F4A2}', '\u{26A0}', '\u{2757}', '\u{1F525}', '\u{1F4A3}',
                ]),
                emoji_density: 0.10,
            },
            Mood::Amused => MoodVisuals {
                body_color: [255, 180, 50],
                head_color: [255, 255, 100],
                speed_multiplier: 0.9,
                emojis: Some(vec![
                    '\u{1F602}', '\u{1F604}', '\u{1F60A}', '\u{1F923}', '\u{1F609}', '\u{1F61C}',
                ]),
                emoji_density: 0.10,
            },
            Mood::Focused => MoodVisuals {
                body_color: [200, 200, 200],
                head_color: [255, 255, 255],
                speed_multiplier: 0.8,
                emojis: Some(vec!['\u{1F3AF}', '\u{2699}', '\u{1F4BB}']),
                emoji_density: 0.05,
            },
            Mood::Serene => MoodVisuals {
                body_color: [0, 220, 200],
                head_color: [150, 255, 240],
                speed_multiplier: 1.4,
                emojis: Some(vec![
                    '\u{1F33F}', '\u{1F33B}', '\u{1F338}', '\u{1F343}', '\u{1F340}', '\u{2618}',
                ]),
                emoji_density: 0.10,
            },
        }
    }
}

/// Incoming mood update from the agent
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MoodUpdate {
    pub mood: Option<Mood>,
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    pub custom: Option<CustomVisuals>,
    pub transition_ms: Option<u64>,
}

fn default_intensity() -> f32 {
    1.0
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CustomVisuals {
    pub body_color: Option<[u8; 3]>,
    pub head_color: Option<[u8; 3]>,
    pub speed_multiplier: Option<f32>,
    pub emojis: Option<String>,
    pub emoji_density: Option<f32>,
}

// --- Color Tween ---

#[derive(Clone)]
pub struct Tween {
    from: [u8; 3],
    to: [u8; 3],
    start: Instant,
    duration: Duration,
}

impl Tween {
    pub fn settled(color: [u8; 3]) -> Self {
        Self {
            from: color,
            to: color,
            start: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    pub fn retarget(&mut self, new_to: [u8; 3], duration: Duration) {
        self.from = self.current();
        self.to = new_to;
        self.start = Instant::now();
        self.duration = duration;
    }

    pub fn current(&self) -> [u8; 3] {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = self.progress();
        lerp_oklab(self.from, self.to, ease_in_out_cubic(t))
    }

    pub fn is_done(&self) -> bool {
        self.start.elapsed() >= self.duration
    }

    fn progress(&self) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (self.start.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
    }
}

// --- Speed Tween ---

#[derive(Clone)]
pub struct SpeedTween {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl SpeedTween {
    pub fn settled(mult: f32) -> Self {
        Self {
            from: mult,
            to: mult,
            start: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    pub fn retarget(&mut self, new_to: f32, duration: Duration) {
        self.from = self.current();
        self.to = new_to;
        self.start = Instant::now();
        self.duration = duration;
    }

    pub fn current(&self) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (self.start.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.0);
        let eased = ease_in_out_cubic(t);
        self.from + (self.to - self.from) * eased
    }

    pub fn is_done(&self) -> bool {
        self.start.elapsed() >= self.duration
    }
}

// --- Emoji Accents ---

/// Manages sparse emoji accents on rain strands
pub struct EmojiAccents {
    current_emojis: Vec<char>,
    target_emojis: Vec<char>,
    current_density: f32,
    target_density: f32,
    /// Transition progress (0.0 = all current, 1.0 = all target)
    progress: f32,
    /// Progress increment per tick
    speed: f32,
}

impl EmojiAccents {
    pub fn new() -> Self {
        Self {
            current_emojis: Vec::new(),
            target_emojis: Vec::new(),
            current_density: 0.0,
            target_density: 0.0,
            progress: 1.0,
            speed: 0.0,
        }
    }

    pub fn tick(&mut self) {
        if self.progress < 1.0 {
            self.progress = (self.progress + self.speed).min(1.0);
            if self.progress >= 1.0 {
                self.current_emojis = self.target_emojis.clone();
                self.current_density = self.target_density;
            }
        }
    }

    pub fn set_target(&mut self, emojis: Vec<char>, density: f32, transition_secs: f32) {
        // Snapshot current interpolated state
        self.current_emojis = if self.progress >= 1.0 {
            self.target_emojis.clone()
        } else if !self.target_emojis.is_empty() {
            self.target_emojis.clone()
        } else {
            self.current_emojis.clone()
        };
        self.current_density =
            self.current_density + (self.target_density - self.current_density) * self.progress;

        self.target_emojis = emojis;
        self.target_density = density.clamp(0.0, 0.25); // cap at 25%
        self.progress = 0.0;
        // At 20fps, we want transition_secs seconds => speed = 1.0 / (transition_secs * 20)
        self.speed = if transition_secs > 0.0 {
            1.0 / (transition_secs * 20.0)
        } else {
            1.0
        };
    }

    pub fn has_emojis(&self) -> bool {
        !self.target_emojis.is_empty() || !self.current_emojis.is_empty()
    }

    /// Get the current effective emoji pool (blended during transitions)
    pub fn current_pool(&self) -> Vec<char> {
        if self.progress >= 1.0 || self.current_emojis.is_empty() {
            self.target_emojis.clone()
        } else if self.target_emojis.is_empty() {
            self.current_emojis.clone()
        } else {
            // Merge both pools during transition
            let mut pool = self.current_emojis.clone();
            for &c in &self.target_emojis {
                if !pool.contains(&c) {
                    pool.push(c);
                }
            }
            pool
        }
    }

    /// Get current interpolated density
    pub fn effective_density(&self) -> f32 {
        self.current_density + (self.target_density - self.current_density) * self.progress
    }
}

// --- MoodDirector ---

/// Manages smooth visual transitions driven by agent mood updates
pub struct MoodDirector {
    pub body_tween: Tween,
    pub head_tween: Tween,
    pub speed_tween: SpeedTween,
    pub emoji_accents: EmojiAccents,
    pub current_mood: Option<Mood>,
    pub intensity: f32,
    base_body: [u8; 3],
    base_head: [u8; 3],
}

impl MoodDirector {
    pub fn new(base_body: [u8; 3], base_head: [u8; 3]) -> Self {
        Self {
            body_tween: Tween::settled(base_body),
            head_tween: Tween::settled(base_head),
            speed_tween: SpeedTween::settled(1.0),
            emoji_accents: EmojiAccents::new(),
            current_mood: None,
            intensity: 0.0,
            base_body,
            base_head,
        }
    }

    /// Update base settings (called when user changes settings).
    /// If a mood is currently active, re-applies it against the new baseline.
    pub fn update_base(&mut self, body: [u8; 3], head: [u8; 3]) {
        self.base_body = body;
        self.base_head = head;

        // Re-apply current mood against the new baseline
        if let Some(mood) = self.current_mood {
            let update = MoodUpdate {
                mood: Some(mood),
                intensity: self.intensity,
                custom: None,
                transition_ms: Some(1000), // quick re-settle
            };
            self.apply_mood(&update);
        }
    }

    /// Apply a mood update from the agent
    pub fn apply_mood(&mut self, update: &MoodUpdate) {
        let duration = Duration::from_millis(update.transition_ms.unwrap_or(2500));
        self.intensity = update.intensity.clamp(0.0, 1.0);
        self.current_mood = update.mood;

        // Resolve target visuals from preset
        let mut visuals = if let Some(mood) = update.mood {
            mood.visuals()
        } else {
            Mood::Neutral.visuals()
        };

        // Apply custom overrides on top of preset
        if let Some(ref custom) = update.custom {
            if let Some(c) = custom.body_color {
                visuals.body_color = c;
            }
            if let Some(c) = custom.head_color {
                visuals.head_color = c;
            }
            if let Some(s) = custom.speed_multiplier {
                visuals.speed_multiplier = s;
            }
            if let Some(ref emoji_str) = custom.emojis {
                visuals.emojis = Some(emoji_str.chars().collect());
            }
            if let Some(d) = custom.emoji_density {
                visuals.emoji_density = d;
            }
        }

        // Apply intensity: lerp between base and target
        let target_body = lerp_oklab(self.base_body, visuals.body_color, self.intensity);
        let target_head = lerp_oklab(self.base_head, visuals.head_color, self.intensity);
        let target_speed = 1.0 + (visuals.speed_multiplier - 1.0) * self.intensity;

        // Retarget tweens (handles mid-transition seamlessly)
        self.body_tween.retarget(target_body, duration);
        self.head_tween.retarget(target_head, duration);
        self.speed_tween
            .retarget(target_speed.clamp(0.3, 3.0), duration);

        // Update emoji accents
        let emoji_chars = visuals.emojis.unwrap_or_default();
        let emoji_density = visuals.emoji_density * self.intensity;
        self.emoji_accents
            .set_target(emoji_chars, emoji_density, duration.as_secs_f32());
    }

    /// Tick emoji accent transitions (call each frame)
    pub fn tick(&mut self) {
        self.emoji_accents.tick();
    }

    /// Get current interpolated body color
    pub fn body_color(&self) -> [u8; 3] {
        self.body_tween.current()
    }

    /// Get current interpolated head color
    pub fn head_color(&self) -> [u8; 3] {
        self.head_tween.current()
    }

    /// Get current speed multiplier
    pub fn speed_multiplier(&self) -> f32 {
        self.speed_tween.current()
    }

    /// Whether any transition is actively in progress
    pub fn is_transitioning(&self) -> bool {
        !self.body_tween.is_done() || !self.head_tween.is_done() || !self.speed_tween.is_done()
    }
}

// --- Oklab color interpolation (inline, no dependency) ---

fn srgb_to_linear(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let v = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

struct OklabColor {
    l: f32,
    a: f32,
    b: f32,
}

fn rgb_to_oklab(rgb: [u8; 3]) -> OklabColor {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);

    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    OklabColor {
        l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    }
}

fn oklab_to_rgb(lab: OklabColor) -> [u8; 3] {
    let l_ = lab.l + 0.3963377774 * lab.a + 0.2158037573 * lab.b;
    let m_ = lab.l - 0.1055613458 * lab.a - 0.0638541728 * lab.b;
    let s_ = lab.l - 0.0894841775 * lab.a - 1.2914855480 * lab.b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    [
        linear_to_srgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        linear_to_srgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        linear_to_srgb(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
    ]
}

pub fn lerp_oklab(from: [u8; 3], to: [u8; 3], t: f32) -> [u8; 3] {
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    let a = rgb_to_oklab(from);
    let b = rgb_to_oklab(to);
    oklab_to_rgb(OklabColor {
        l: a.l + (b.l - a.l) * t,
        a: a.a + (b.a - a.a) * t,
        b: a.b + (b.b - a.b) * t,
    })
}

fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tween_settled_returns_color() {
        let tween = Tween::settled([0, 255, 0]);
        assert_eq!(tween.current(), [0, 255, 0]);
        assert!(tween.is_done());
    }

    #[test]
    fn tween_retarget_mid_transition() {
        let mut tween = Tween::settled([0, 0, 0]);
        tween.retarget([255, 255, 255], Duration::from_secs(10));
        assert!(!tween.is_done());
        // Immediately after retarget, should still be near [0,0,0]
        let c = tween.current();
        assert!(c[0] < 30 && c[1] < 30 && c[2] < 30);
    }

    #[test]
    fn lerp_oklab_endpoints() {
        assert_eq!(lerp_oklab([0, 0, 0], [255, 255, 255], 0.0), [0, 0, 0]);
        assert_eq!(
            lerp_oklab([0, 0, 0], [255, 255, 255], 1.0),
            [255, 255, 255]
        );
    }

    #[test]
    fn lerp_oklab_midpoint_not_muddy() {
        let mid = lerp_oklab([0, 255, 0], [255, 0, 200], 0.5);
        // Midpoint should have reasonable brightness (not near black)
        let brightness = mid[0] as u16 + mid[1] as u16 + mid[2] as u16;
        assert!(brightness > 100, "midpoint too dark: {mid:?}");
    }

    #[test]
    fn mood_director_apply_preset() {
        let mut director = MoodDirector::new([0, 255, 0], [255, 255, 255]);
        director.apply_mood(&MoodUpdate {
            mood: Some(Mood::Excited),
            intensity: 1.0,
            custom: None,
            transition_ms: Some(0), // instant for test
        });
        // After instant transition, body should be excited color
        let body = director.body_color();
        // Excited = [255, 50, 200] — should be close
        assert!(body[0] > 200, "expected red-ish body: {body:?}");
    }

    #[test]
    fn mood_director_apply_custom() {
        let mut director = MoodDirector::new([0, 255, 0], [255, 255, 255]);
        director.apply_mood(&MoodUpdate {
            mood: None,
            intensity: 1.0,
            custom: Some(CustomVisuals {
                body_color: Some([100, 100, 100]),
                head_color: None,
                speed_multiplier: None,
                emojis: Some("\u{1F916}\u{1F9BE}".to_string()),
                emoji_density: Some(0.15),
            }),
            transition_ms: Some(0),
        });
        let body = director.body_color();
        assert_eq!(body, [100, 100, 100]);
        assert!(director.emoji_accents.has_emojis());
    }

    #[test]
    fn speed_tween_clamps() {
        let mut director = MoodDirector::new([0, 255, 0], [255, 255, 255]);
        director.apply_mood(&MoodUpdate {
            mood: Some(Mood::Contemplative),
            intensity: 1.0,
            custom: None,
            transition_ms: Some(0),
        });
        let speed = director.speed_multiplier();
        assert!(speed >= 0.3 && speed <= 3.0, "speed out of range: {speed}");
    }

    #[test]
    fn ease_in_out_cubic_boundaries() {
        assert!((ease_in_out_cubic(0.0) - 0.0).abs() < 0.001);
        assert!((ease_in_out_cubic(1.0) - 1.0).abs() < 0.001);
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 0.001);
    }
}
