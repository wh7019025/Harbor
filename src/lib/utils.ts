import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function clampPercent(value: number | null | undefined) {
  if (value == null || Number.isNaN(value)) return null;
  return Math.min(100, Math.max(0, value));
}

export function formatPercent(value: number | null | undefined) {
  if (value == null) return "--";
  return `${value.toFixed(1)}%`;
}

export function formatBytes(value: number | null | undefined) {
  if (value == null) return "--";
  if (value === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let scaled = value;
  let unitIndex = 0;
  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }
  return `${scaled >= 10 ? scaled.toFixed(0) : scaled.toFixed(1)} ${units[unitIndex]}`;
}

export function formatBytesPerSecond(value: number | null | undefined) {
  if (value == null) return "--";
  return `${formatBytes(value)}/s`;
}

export function formatFrequency(value: number | null | undefined) {
  if (value == null) return "--";
  if (value >= 1000) return `${(value / 1000).toFixed(2)} GHz`;
  return `${value.toFixed(0)} MHz`;
}
