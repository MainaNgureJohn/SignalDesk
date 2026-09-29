#!/usr/bin/env node

const endpoint = "https://agent.binance.com/mcp/agentic";
let requestId = 1;
let sessionId;
const bearerToken = process.env.BINANCE_MCP_TOKEN?.trim();

function parseMessage(text, contentType) {
  if (contentType.includes("application/json")) return JSON.parse(text);
  const messages = text
    .split(/\r?\n/)
    .filter((line) => line.startsWith("data:"))
    .map((line) => JSON.parse(line.slice(5).trim()));
  return messages.at(-1);
}

async function send(body) {
  const headers = {
    Accept: "application/json, text/event-stream",
    "Content-Type": "application/json",
  };
  if (sessionId) headers["Mcp-Session-Id"] = sessionId;
  if (bearerToken) headers.Authorization = `Bearer ${bearerToken}`;
  const response = await fetch(endpoint, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  sessionId ||= response.headers.get("mcp-session-id") ?? undefined;
  const text = await response.text();
  if (!response.ok) {
    const challenge = response.headers.get("www-authenticate");
    if (response.status === 401 && challenge) {
      throw new Error(
        `HTTP 401: Binance requires an OAuth MCP session (${challenge}). ` +
          "Authenticate through Codex with `codex mcp login binance`, or set " +
          "BINANCE_MCP_TOKEN for this diagnostic process.",
      );
    }
    throw new Error(`HTTP ${response.status}: ${text.slice(0, 500)}`);
  }
  if (!text.trim()) return null;
  return parseMessage(text, response.headers.get("content-type") ?? "");
}

async function request(method, params = {}) {
  const id = requestId++;
  const message = await send({ jsonrpc: "2.0", id, method, params });
  if (!message || message.id !== id) {
    throw new Error(`Missing response for ${method}`);
  }
  if (message.error) throw new Error(`${method}: ${JSON.stringify(message.error)}`);
  return message.result;
}

async function main() {
  await request("initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "signaldesk-smoke", version: "0.1.0" },
  });
  await send({ jsonrpc: "2.0", method: "notifications/initialized", params: {} });

  const listed = await request("tools/list");
  const tools = listed.tools ?? [];
  const publicTools = tools.filter((tool) =>
    /ticker|price|market|symbol/i.test(`${tool.name} ${tool.description ?? ""}`),
  );

  console.log(
    JSON.stringify(
      { endpoint, authenticated: true, sessionId: sessionId ?? null, publicTools },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
