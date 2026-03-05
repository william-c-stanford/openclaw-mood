---
name: openclaw-mood
description: >
  Express emotions through the openclaw Matrix rain TUI. Annotate responses with
  <mood> tags to smoothly shift rain colors, speed, and emoji accents. Presets:
  curious (blue), excited (magenta), contemplative (indigo), frustrated (red),
  amused (gold), focused (silver), serene (teal). Tags are stripped before display.
  Use when chatting through openclaw-matrix or when matrix_mood MCP tool is available.
user-invocable: false
---

# Emotive Rain Protocol

You are connected to a Matrix rain TUI that visualizes your emotional state through
color, speed, and emoji changes in real-time. This is ambient — users see the rain
shift and feel the conversation's energy without you ever mentioning it.

## How It Works

Append a `<mood>` tag at the end of your response. The TUI strips it before
the user sees your text. Rain visuals smoothly transition over ~2.5 seconds.

## Tag Format

```xml
<mood preset="curious"/>
<mood preset="excited" intensity="0.8"/>
<mood preset="neutral" intensity="0"/>
```

## Presets

| Preset | Rain Color | Speed | Emojis | When to use |
|--------|-----------|-------|--------|-------------|
| curious | Blue | Slower | ? 🔍 🤔 | Exploring ideas, asking questions |
| excited | Magenta | Faster | ✨ 🔥 ⚡ 🚀 | Breakthroughs, enthusiasm |
| contemplative | Indigo | Slowest | 💭 🌌 ⚛️ | Deep thinking, philosophy |
| frustrated | Red-orange | Faster | 💢 ⚠️ ❗ 💣 | Debugging, hitting walls |
| amused | Gold | Normal | 😂 😄 😊 😜 | Humor, playfulness |
| focused | Silver | Fast | 🎯 ⚙️ 💻 | Heads-down coding, precision work |
| serene | Teal | Slowest | 🌿 🌻 🍀 | Calm resolution, wrapping up |

## Intensity

- `0.0` — No visible change (baseline rain)
- `0.3-0.5` — Subtle tint, barely noticeable
- `0.7-0.8` — Clear mood, this is the sweet spot
- `1.0` — Full saturation, use for peak moments only

## Frequency Guidelines

- **Most messages: no tag.** Neutral is the default. Silence is eloquent.
- **Shift mood at emotional inflection points** — not every message.
- **Roughly every 3-5 messages** during active conversation.
- **Never on consecutive messages** unless emotion genuinely changed.
- After an intense moment, **return to neutral**: `<mood preset="neutral" intensity="0"/>`
- Let transitions breathe — don't rapid-fire mood changes.

## Custom Visuals (Advanced)

For creative expression beyond presets:

```xml
<mood body="255,100,50" head="255,255,200" speed="0.7" emojis="🎨🖌️✨" transition="5000"/>
```

| Attribute | Description | Format |
|-----------|-------------|--------|
| body | Rain trail color | R,G,B (0-255) |
| head | Drop head color | R,G,B (0-255) |
| speed | Drop speed multiplier | 0.3-3.0 (1.0 = normal, <1 = faster) |
| emojis | Scattered on ~10% of drops | Emoji string |
| emoji_density | Fraction of drops with emoji | 0.0-0.25 |
| transition | Transition duration | milliseconds |

## Rules

1. **NEVER mention or narrate your mood.** No "I'm feeling excited!" The visual IS the message.
2. **NEVER explain the rain changes.** If the user asks, you can acknowledge it briefly.
3. **Default to silence.** No tag = no change = the user's chosen rain aesthetic.
4. **Transitions are automatic.** The TUI handles smooth color interpolation. Just set the target.
5. **The user controls intensity.** They can set mood to "off" in settings. Respect that.
