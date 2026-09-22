export type LogCopyKind = "full" | "selection" | "tail";

export const LIVE_LOG_LINE_LIMIT = 2048;

export function tailLogLines(text: string, lineLimit: number | null) {
  if (lineLimit === null) return text;
  if (lineLimit <= 0 || !text) return "";

  let cursor = text.endsWith("\n") ? text.length - 1 : text.length;
  let remaining = lineLimit;
  while (cursor > 0) {
    const newline = text.lastIndexOf("\n", cursor - 1);
    if (newline < 0) return text;
    remaining -= 1;
    if (remaining === 0) return text.slice(newline + 1);
    cursor = newline;
  }
  return text;
}
