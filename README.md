# openclaw-matrix

Matrix rain TUI for [openclaw](https://github.com/william-c-stanford/openclaw) — chat with your AI agent while rain streams in the background. The rain responds to the agent's emotional state with smooth color transitions, speed changes, and emoji accents.

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Connect to local openclaw gateway
openclaw-matrix --gateway-url ws://localhost:18789/ws

# Screensaver mode (no agent, just rain)
openclaw-matrix --offline

# Custom colors
openclaw-matrix --color "0,255,255" --head "#FF00FF"
```

### Controls

| Key | Action |
|-----|--------|
| `i` or `/` | Start typing |
| `Enter` | Send message |
| `Esc` | Back to viewing / quit |
| `m` | Cycle mood presets (debug) |
| `Ctrl+S` | Settings panel |
| `Up/Down` | Scroll chat |

### Settings

Press `Ctrl+S` to open the settings panel. Configure:

- **Color** — Rain body color
- **Head** — Leading character color
- **Group** — Character set (katakana, binary, emoji, etc.)
- **Direction** — Rain fall direction
- **Speed** — Drop speed range
- **Shade** — Enable/disable color shading
- **Gradient** — Shade gradient base color
- **Mood** — Agent mood frequency: off, rare, normal, expressive

Settings persist across restarts in `~/.config/openclaw-matrix/settings.toml`.

## Emotive Rain

The agent can express emotions through the rain by including `<mood>` tags in its responses. The tags are stripped before display — the visual change IS the communication.

### Presets

| Preset | Color | Emojis | When |
|--------|-------|--------|------|
| curious | Blue | ? 🔍 🤔 | Exploring ideas |
| excited | Magenta | ✨ 🔥 ⚡ 🚀 | Breakthroughs |
| contemplative | Indigo | 💭 🌌 ⚛️ | Deep thinking |
| frustrated | Red-orange | 💢 ⚠️ ❗ | Debugging |
| amused | Gold | 😂 😄 😊 | Humor |
| focused | Silver | 🎯 ⚙️ 💻 | Coding |
| serene | Teal | 🌿 🌻 🍀 | Wrapping up |

### For Claude Code Users

Install the MCP bridge and skill:

```bash
npx @openclaw/matrix-bridge install
```

This installs:
1. A skill file (`~/.claude/skills/openclaw-mood/SKILL.md`) that teaches Claude the mood protocol
2. An MCP server config for the `matrix_mood` tool (direct visual control)

### For Other LLM Agents

Add this to your agent's system prompt:

```
You can express emotions through the Matrix rain by including a <mood> tag
at the end of your response. The tag is stripped before display.

Presets: curious (blue), excited (magenta), contemplative (indigo),
frustrated (red), amused (gold), focused (silver), serene (teal).

Format: <mood preset="curious" intensity="0.8"/>
Reset:  <mood preset="neutral" intensity="0"/>

Guidelines:
- Use mood at natural emotional transitions, roughly every 3-5 messages
- Omit the tag entirely when neutral (most messages)
- Never narrate or explain your mood — the visual change IS the communication
- Intensity 0.3-0.5 for subtle, 0.7-1.0 for strong emotions
```

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full system documentation.

## License

MIT
