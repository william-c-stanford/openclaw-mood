#!/usr/bin/env node

/**
 * @openclaw/matrix-bridge — MCP server for controlling openclaw-matrix TUI mood visuals.
 *
 * Provides two tools:
 *   - matrix_mood: Set the rain mood (colors, speed, emojis)
 *   - matrix_status: Check TUI connection state and current mood
 *
 * Connects to the TUI's gateway WebSocket to send mood.update JSON-RPC frames.
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import WebSocket from "ws";

const GATEWAY_URL =
  process.env.OPENCLAW_GATEWAY_URL || "ws://localhost:18789/ws";

// --- WebSocket connection to the TUI gateway ---

let ws: WebSocket | null = null;
let currentMood: string | null = null;
let connected = false;

function connectToGateway(): void {
  if (ws && ws.readyState === WebSocket.OPEN) return;

  try {
    ws = new WebSocket(GATEWAY_URL);

    ws.on("open", () => {
      connected = true;
      console.error(`[matrix-bridge] Connected to gateway: ${GATEWAY_URL}`);
    });

    ws.on("close", () => {
      connected = false;
      console.error("[matrix-bridge] Gateway disconnected");
      // Reconnect after 5s
      setTimeout(connectToGateway, 5000);
    });

    ws.on("error", (err) => {
      console.error(`[matrix-bridge] WebSocket error: ${err.message}`);
      connected = false;
    });

    ws.on("message", (data) => {
      // We mostly send, but log incoming for debugging
      console.error(`[matrix-bridge] Received: ${data.toString().slice(0, 200)}`);
    });
  } catch (err) {
    console.error(`[matrix-bridge] Connection failed: ${err}`);
    connected = false;
  }
}

function sendMoodUpdate(params: Record<string, unknown>): boolean {
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    return false;
  }

  const frame = JSON.stringify({
    jsonrpc: "3.0",
    method: "mood.update",
    params,
  });

  ws.send(frame);
  return true;
}

// --- MCP Server ---

const server = new McpServer({
  name: "openclaw-matrix-bridge",
  version: "0.1.0",
});

server.registerTool(
  "matrix_mood",
  {
    title: "Set Matrix Rain Mood",
    description:
      "Set the matrix rain mood in the openclaw TUI. Use sparingly for creative " +
      "visual expression — custom colors, emojis, or intensity that presets don't cover. " +
      "For standard moods (curious, excited, focused, etc.), prefer using <mood> tags " +
      "in your response text instead. Only call this tool when you want full creative control.",
    inputSchema: z.object({
      mood: z
        .enum([
          "neutral",
          "curious",
          "excited",
          "contemplative",
          "frustrated",
          "amused",
          "focused",
          "serene",
        ])
        .optional()
        .describe(
          "Preset mood. Omit for fully custom visuals."
        ),
      intensity: z
        .number()
        .min(0)
        .max(1)
        .optional()
        .describe(
          "How strongly to express the mood. 0 = user's base, 1 = full mood. Default 0.8."
        ),
      body_color: z
        .tuple([z.number().int().min(0).max(255), z.number().int().min(0).max(255), z.number().int().min(0).max(255)])
        .optional()
        .describe("Custom RGB body color, e.g. [255, 100, 50]. Overrides preset."),
      head_color: z
        .tuple([z.number().int().min(0).max(255), z.number().int().min(0).max(255), z.number().int().min(0).max(255)])
        .optional()
        .describe("Custom RGB head color."),
      emojis: z
        .string()
        .optional()
        .describe(
          "Emoji characters scattered as rain drop heads, e.g. '🤖🦾🧠'. Overrides preset."
        ),
      transition_ms: z
        .number()
        .int()
        .min(100)
        .max(10000)
        .optional()
        .describe(
          "Transition duration in ms. Default 2500. Use 500 for snappy, 5000+ for gradual."
        ),
    }),
  },
  async (args) => {
    // Build mood.update params
    const params: Record<string, unknown> = {};

    if (args.mood) {
      params.mood = args.mood;
    }
    params.intensity = args.intensity ?? 0.8;

    if (args.transition_ms) {
      params.transition_ms = args.transition_ms;
    }

    // Custom overrides
    const custom: Record<string, unknown> = {};
    if (args.body_color) custom.body_color = args.body_color;
    if (args.head_color) custom.head_color = args.head_color;
    if (args.emojis) custom.emojis = args.emojis;

    if (Object.keys(custom).length > 0) {
      params.custom = custom;
    }

    // Track current mood
    currentMood = args.mood || "custom";

    // Send to TUI
    const sent = sendMoodUpdate(params);

    if (!sent) {
      return {
        content: [
          {
            type: "text" as const,
            text: "Failed to send mood update — TUI is not connected. Make sure openclaw-matrix is running.",
          },
        ],
        isError: true,
      };
    }

    const moodDesc = args.mood
      ? `Mood set to "${args.mood}" at intensity ${params.intensity}`
      : `Custom mood applied at intensity ${params.intensity}`;

    return {
      content: [
        {
          type: "text" as const,
          text: moodDesc,
        },
      ],
    };
  }
);

server.registerTool(
  "matrix_status",
  {
    title: "Matrix Rain Status",
    description:
      "Check if the openclaw-matrix TUI is connected and what mood is active. " +
      "Call this once at the start of a conversation to know if visual mood is available.",
    inputSchema: z.object({}),
  },
  async () => {
    const status = {
      connected,
      gateway_url: GATEWAY_URL,
      current_mood: currentMood,
    };

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(status, null, 2),
        },
      ],
    };
  }
);

// --- Start ---

async function main() {
  // Try to connect to gateway (non-blocking — works even if TUI isn't running yet)
  connectToGateway();

  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("openclaw-matrix-bridge MCP server running on stdio");
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
