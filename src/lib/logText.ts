import { AnsiUp } from "ansi_up";

const ansiUp = new AnsiUp();

/** Normalize line endings; drop control chars except TAB, LF, and ESC (for ANSI). */
export function normalizeLogText(text: string): string {
  return text
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .replace(/[\x00-\x08\x0B\x0C\x0E-\x1A\x1C-\x1F\x7F]/g, "");
}

/** Render task log text with ANSI colors as HTML. */
export function logToHtml(text: string): string {
  return ansiUp.ansi_to_html(normalizeLogText(text));
}

/** Plain-text fallback without ANSI or control characters. */
export function sanitizeLogText(text: string): string {
  const ANSI_ESCAPE = /\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])/g;
  return normalizeLogText(text).replace(ANSI_ESCAPE, "");
}
