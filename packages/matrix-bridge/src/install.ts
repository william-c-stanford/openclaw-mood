#!/usr/bin/env node

/**
 * Install/uninstall script for @openclaw/matrix-bridge.
 *
 * Usage:
 *   npx @openclaw/matrix-bridge install    — sets up skill + MCP server
 *   npx @openclaw/matrix-bridge uninstall  — removes skill + MCP server config
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync, rmSync } from "fs";
import { join } from "path";
import { homedir } from "os";
import { fileURLToPath } from "url";
import { dirname } from "path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const SKILL_DIR = join(homedir(), ".claude", "skills", "openclaw-mood");
const SKILL_SRC = join(__dirname, "..", "skill", "SKILL.md");
const SETTINGS_PATH = join(homedir(), ".claude", "settings.json");

const MCP_SERVER_KEY = "openclaw-matrix";

function install() {
  console.log("Installing @openclaw/matrix-bridge...\n");

  // 1. Install skill file
  if (!existsSync(SKILL_DIR)) {
    mkdirSync(SKILL_DIR, { recursive: true });
  }
  const skillContent = readFileSync(SKILL_SRC, "utf-8");
  writeFileSync(join(SKILL_DIR, "SKILL.md"), skillContent);
  console.log("  [ok] Installed skill: ~/.claude/skills/openclaw-mood/SKILL.md");

  // 2. Add MCP server to settings.json
  let settings: Record<string, unknown> = {};
  if (existsSync(SETTINGS_PATH)) {
    try {
      settings = JSON.parse(readFileSync(SETTINGS_PATH, "utf-8"));
    } catch {
      console.warn("  [warn] Could not parse existing settings.json, creating new one");
    }
  }

  const mcpServers = (settings.mcpServers || {}) as Record<string, unknown>;
  mcpServers[MCP_SERVER_KEY] = {
    command: "npx",
    args: ["-y", "@openclaw/matrix-bridge"],
    env: {
      OPENCLAW_GATEWAY_URL: "ws://localhost:18789/ws",
    },
  };
  settings.mcpServers = mcpServers;

  // Ensure parent directory exists
  const settingsDir = dirname(SETTINGS_PATH);
  if (!existsSync(settingsDir)) {
    mkdirSync(settingsDir, { recursive: true });
  }
  writeFileSync(SETTINGS_PATH, JSON.stringify(settings, null, 2) + "\n");
  console.log("  [ok] Added MCP server to ~/.claude/settings.json");

  console.log("\nDone! To use:\n");
  console.log("  1. Start openclaw-matrix:  cargo run");
  console.log("  2. Start Claude Code in the same project");
  console.log("  3. The agent will auto-detect the mood protocol\n");
  console.log("  The rain will shift colors as the agent responds.");
  console.log("  Use settings (Ctrl+S) to adjust mood frequency.\n");
}

function uninstall() {
  console.log("Uninstalling @openclaw/matrix-bridge...\n");

  // 1. Remove skill directory
  if (existsSync(SKILL_DIR)) {
    rmSync(SKILL_DIR, { recursive: true });
    console.log("  [ok] Removed skill: ~/.claude/skills/openclaw-mood/");
  } else {
    console.log("  [skip] Skill directory not found");
  }

  // 2. Remove MCP server from settings.json
  if (existsSync(SETTINGS_PATH)) {
    try {
      const settings = JSON.parse(readFileSync(SETTINGS_PATH, "utf-8"));
      const mcpServers = settings.mcpServers || {};
      if (MCP_SERVER_KEY in mcpServers) {
        delete mcpServers[MCP_SERVER_KEY];
        settings.mcpServers = mcpServers;
        writeFileSync(SETTINGS_PATH, JSON.stringify(settings, null, 2) + "\n");
        console.log("  [ok] Removed MCP server from ~/.claude/settings.json");
      } else {
        console.log("  [skip] MCP server not found in settings");
      }
    } catch {
      console.warn("  [warn] Could not parse settings.json");
    }
  }

  console.log("\nDone! @openclaw/matrix-bridge has been uninstalled.\n");
}

const command = process.argv[2];

switch (command) {
  case "install":
    install();
    break;
  case "uninstall":
    uninstall();
    break;
  default:
    // If no subcommand, this is being run as MCP server — delegate to index.ts
    // (handled by package.json bin entry)
    console.error("Usage: npx @openclaw/matrix-bridge [install|uninstall]");
    console.error("  install    — Install skill + MCP server config");
    console.error("  uninstall  — Remove skill + MCP server config");
    process.exit(1);
}
