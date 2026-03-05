//! Client-side extraction of `<mood .../>` tags from streamed LLM output.
//!
//! The LLM annotates responses with inline mood tags like:
//!   `<mood preset="curious" intensity="0.8"/>`
//! This module strips them from the display text and returns parsed MoodUpdates.

use crate::mood::{CustomVisuals, MoodUpdate};

/// Extract all complete `<mood .../>` tags from text.
/// Returns (cleaned_text, mood_updates).
pub fn extract_mood_tags(text: &str) -> (String, Vec<MoodUpdate>) {
    let mut cleaned = String::with_capacity(text.len());
    let mut updates = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<mood") {
        // Add text before the tag
        cleaned.push_str(&remaining[..start]);

        let after_open = &remaining[start..];
        // Find the closing `/>` or `>`
        if let Some(end) = find_tag_end(after_open) {
            let tag_str = &after_open[..end];
            if let Some(update) = parse_mood_tag(tag_str) {
                updates.push(update);
            } else {
                eprintln!("[mood_tag] malformed tag, stripping: {tag_str}");
            }
            remaining = &after_open[end..];
        } else {
            // Incomplete tag — keep it (will complete on next delta)
            cleaned.push_str(after_open);
            remaining = "";
        }
    }

    cleaned.push_str(remaining);
    (cleaned, updates)
}

/// Check if the text ends with an incomplete `<mood` tag.
/// Returns the index where the potential partial tag starts, or None.
pub fn has_partial_mood_tag(text: &str) -> Option<usize> {
    // Check for partial `<mood` at the end of the buffer
    // Could be `<`, `<m`, `<mo`, `<moo`, `<mood`, or `<mood ...` without closing
    let needle = "<mood";
    for suffix_len in (1..=needle.len()).rev() {
        let suffix = &needle[..suffix_len];
        if text.ends_with(suffix) {
            return Some(text.len() - suffix_len);
        }
    }

    // Also check for `<mood ...` that started but no `/>` yet
    if let Some(start) = text.rfind("<mood") {
        let after = &text[start..];
        if find_tag_end(after).is_none() {
            return Some(start);
        }
    }

    None
}

/// Find the end of a `<mood .../>` or `<mood ...>` tag.
/// Returns the byte offset AFTER the closing `>`.
fn find_tag_end(tag_start: &str) -> Option<usize> {
    // Look for `/>` or `>` after `<mood`
    let mut in_quote = false;
    let mut quote_char = '"';
    for (i, ch) in tag_start.char_indices().skip(1) {
        match ch {
            '"' | '\'' if !in_quote => {
                in_quote = true;
                quote_char = ch;
            }
            c if in_quote && c == quote_char => {
                in_quote = false;
            }
            '>' if !in_quote => {
                return Some(i + 1);
            }
            _ => {}
        }
    }
    None
}

/// Parse a `<mood .../>` tag string into a MoodUpdate.
fn parse_mood_tag(tag: &str) -> Option<MoodUpdate> {
    // Extract attributes from the tag
    let attrs = extract_attributes(tag);

    let preset_str = attrs.get("preset");
    let mood = preset_str.and_then(|s| {
        // Parse mood preset using serde
        let json = format!("\"{}\"", s);
        serde_json::from_str(&json).ok()
    });

    let intensity = attrs
        .get("intensity")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(if mood.is_some() { 1.0 } else { 0.0 });

    let transition_ms = attrs
        .get("transition")
        .and_then(|s| s.parse::<u64>().ok());

    // Custom visuals from inline attributes
    let body_color = attrs.get("body").and_then(|s| parse_rgb(s));
    let head_color = attrs.get("head").and_then(|s| parse_rgb(s));
    let speed_multiplier = attrs.get("speed").and_then(|s| s.parse::<f32>().ok());
    let emojis = attrs.get("emojis").map(|s| s.to_string());
    let emoji_density = attrs
        .get("emoji_density")
        .and_then(|s| s.parse::<f32>().ok());

    let custom = if body_color.is_some()
        || head_color.is_some()
        || speed_multiplier.is_some()
        || emojis.is_some()
        || emoji_density.is_some()
    {
        Some(CustomVisuals {
            body_color,
            head_color,
            speed_multiplier,
            emojis,
            emoji_density,
        })
    } else {
        None
    };

    Some(MoodUpdate {
        mood,
        intensity,
        custom,
        transition_ms,
    })
}

/// Parse "R,G,B" string into [u8; 3]
fn parse_rgb(s: &str) -> Option<[u8; 3]> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    Some([r, g, b])
}

/// Extract key="value" attributes from a tag string.
fn extract_attributes(tag: &str) -> std::collections::HashMap<String, String> {
    let mut attrs = std::collections::HashMap::new();

    // Skip `<mood ` prefix
    let content = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/')
        .trim();
    let content = content.strip_prefix("mood").unwrap_or(content).trim();

    let mut chars = content.chars().peekable();
    while chars.peek().is_some() {
        // Skip whitespace
        while chars.peek().map_or(false, |c| c.is_whitespace()) {
            chars.next();
        }

        // Read key
        let key: String = chars
            .by_ref()
            .take_while(|c| *c != '=')
            .collect::<String>()
            .trim()
            .to_string();

        if key.is_empty() {
            break;
        }

        // Skip whitespace and opening quote
        while chars.peek().map_or(false, |c| c.is_whitespace()) {
            chars.next();
        }
        let quote = match chars.peek() {
            Some('"') | Some('\'') => {
                let q = *chars.peek().unwrap();
                chars.next();
                q
            }
            _ => {
                // Unquoted value — read until whitespace or end
                let value: String = chars
                    .by_ref()
                    .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>')
                    .collect();
                attrs.insert(key, value);
                continue;
            }
        };

        // Read value until closing quote
        let value: String = chars.by_ref().take_while(|c| *c != quote).collect();
        attrs.insert(key, value);
    }

    attrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_preset_only() {
        let (cleaned, updates) =
            extract_mood_tags("Hello world!<mood preset=\"curious\"/>");
        assert_eq!(cleaned, "Hello world!");
        assert_eq!(updates.len(), 1);
        assert!(updates[0].mood.is_some());
    }

    #[test]
    fn extract_preset_with_intensity() {
        let (cleaned, updates) =
            extract_mood_tags("Great!<mood preset=\"excited\" intensity=\"0.8\"/>");
        assert_eq!(cleaned, "Great!");
        assert_eq!(updates.len(), 1);
        assert!((updates[0].intensity - 0.8).abs() < 0.01);
    }

    #[test]
    fn extract_custom_visuals() {
        let (cleaned, updates) = extract_mood_tags(
            "Art!<mood body=\"255,100,50\" head=\"255,255,200\" emojis=\"🎨🖌️\" transition=\"5000\"/>",
        );
        assert_eq!(cleaned, "Art!");
        assert_eq!(updates.len(), 1);
        let custom = updates[0].custom.as_ref().unwrap();
        assert_eq!(custom.body_color, Some([255, 100, 50]));
        assert_eq!(custom.head_color, Some([255, 255, 200]));
        assert_eq!(updates[0].transition_ms, Some(5000));
    }

    #[test]
    fn no_tag() {
        let (cleaned, updates) = extract_mood_tags("Just normal text");
        assert_eq!(cleaned, "Just normal text");
        assert!(updates.is_empty());
    }

    #[test]
    fn tag_in_middle() {
        let (cleaned, updates) =
            extract_mood_tags("before<mood preset=\"focused\"/>after");
        assert_eq!(cleaned, "beforeafter");
        assert_eq!(updates.len(), 1);
    }

    #[test]
    fn partial_tag_detected() {
        assert!(has_partial_mood_tag("some text <mood").is_some());
        assert!(has_partial_mood_tag("some text <moo").is_some());
        assert!(has_partial_mood_tag("some text <m").is_some());
        assert!(has_partial_mood_tag("some text <mood preset=\"curious\"").is_some());
        assert!(has_partial_mood_tag("complete <mood preset=\"x\"/>").is_none());
        assert!(has_partial_mood_tag("no tag here").is_none());
    }

    #[test]
    fn neutral_reset() {
        let (_, updates) =
            extract_mood_tags("<mood preset=\"neutral\" intensity=\"0\"/>");
        assert_eq!(updates.len(), 1);
        assert!((updates[0].intensity - 0.0).abs() < 0.01);
    }
}
