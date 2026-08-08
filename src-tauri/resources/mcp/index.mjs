#!/usr/bin/env node
/**
 * Harbor MCP Resources (zero-deps).
 * Exposes ~/.harbor/agent_doc/** as harbor://agent_doc/<relpath>
 */
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const HARBOR_HOME = path.join(os.homedir(), ".harbor");
const DOC_ROOT = path.join(HARBOR_HOME, "agent_doc");
const VERSION_FILE = path.join(HARBOR_HOME, "version.json");

function mimeFor(file) {
  if (file.endsWith(".md")) return "text/markdown";
  if (file.endsWith(".json")) return "application/json";
  if (file.endsWith(".yaml") || file.endsWith(".yml")) return "text/yaml";
  return "text/plain";
}

function toPosix(rel) {
  return rel.split(path.sep).join("/");
}

async function loadVersions() {
  try {
    const raw = await fs.readFile(VERSION_FILE, "utf8");
    return JSON.parse(raw);
  } catch {
    return { app: "0.0.0" };
  }
}

async function walkDocs(dir, base = DOC_ROOT) {
  const out = [];
  let entries;
  try {
    entries = await fs.readdir(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...(await walkDocs(full, base)));
      continue;
    }
    if (!entry.isFile()) continue;
    out.push(toPosix(path.relative(base, full)));
  }
  return out;
}

async function listDocs() {
  const files = await walkDocs(DOC_ROOT);
  files.sort();
  return files;
}

/** Resolve URI path under DOC_ROOT; reject traversal. */
function resolveDocPath(relUriPath) {
  if (!relUriPath || relUriPath.includes("\0")) return null;
  const decoded = decodeURIComponent(relUriPath);
  if (path.isAbsolute(decoded)) return null;
  const parts = decoded.split(/[/\\]/).filter((p) => p.length > 0);
  if (parts.some((p) => p === "..")) return null;
  const joined = path.resolve(DOC_ROOT, ...parts);
  const root = path.resolve(DOC_ROOT);
  if (joined !== root && !joined.startsWith(root + path.sep)) return null;
  return joined;
}

function send(message) {
  const body = JSON.stringify(message);
  const payload = `Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n${body}`;
  process.stdout.write(payload);
}

async function handle(message) {
  const { id, method, params } = message;
  if (method === "initialize") {
    const versions = await loadVersions();
    return {
      jsonrpc: "2.0",
      id,
      result: {
        protocolVersion: params?.protocolVersion || "2024-11-05",
        capabilities: { resources: { listChanged: false } },
        serverInfo: {
          name: "harbor",
          version: versions.app || "0.0.0",
        },
      },
    };
  }
  if (method === "notifications/initialized" || method === "initialized") {
    return null;
  }
  if (method === "ping") {
    return { jsonrpc: "2.0", id, result: {} };
  }
  if (method === "resources/list") {
    const files = await listDocs();
    return {
      jsonrpc: "2.0",
      id,
      result: {
        resources: files.map((file) => ({
          uri: `harbor://agent_doc/${file}`,
          name: file,
          mimeType: mimeFor(file),
        })),
      },
    };
  }
  if (method === "resources/read") {
    const uri = params?.uri || "";
    const prefix = "harbor://agent_doc/";
    if (!uri.startsWith(prefix)) {
      return {
        jsonrpc: "2.0",
        id,
        error: { code: -32602, message: `unsupported uri: ${uri}` },
      };
    }
    const rel = uri.slice(prefix.length);
    const filePath = resolveDocPath(rel);
    if (!filePath) {
      return {
        jsonrpc: "2.0",
        id,
        error: { code: -32602, message: `invalid resource path: ${uri}` },
      };
    }
    try {
      const text = await fs.readFile(filePath, "utf8");
      const name = toPosix(path.relative(DOC_ROOT, filePath));
      return {
        jsonrpc: "2.0",
        id,
        result: {
          contents: [{ uri, mimeType: mimeFor(name), text }],
        },
      };
    } catch (error) {
      return {
        jsonrpc: "2.0",
        id,
        error: { code: -32002, message: String(error) },
      };
    }
  }
  if (id === undefined) return null;
  return {
    jsonrpc: "2.0",
    id,
    error: { code: -32601, message: `Method not found: ${method}` },
  };
}

let buffer = Buffer.alloc(0);

process.stdin.on("data", async (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) return;
    const header = buffer.slice(0, headerEnd).toString("utf8");
    const match = /Content-Length:\s*(\d+)/i.exec(header);
    if (!match) {
      buffer = buffer.slice(headerEnd + 4);
      continue;
    }
    const length = Number(match[1]);
    const start = headerEnd + 4;
    const end = start + length;
    if (buffer.length < end) return;
    const body = buffer.slice(start, end).toString("utf8");
    buffer = buffer.slice(end);
    let message;
    try {
      message = JSON.parse(body);
    } catch (error) {
      console.error("invalid json", error);
      continue;
    }
    const response = await handle(message);
    if (response) send(response);
  }
});

process.stdin.on("end", () => process.exit(0));
