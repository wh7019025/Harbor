//! Shared Harbor GUI and Core logging.

use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::settings::harbor_log_path;

const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;
const KEEP_LOG_BYTES: u64 = 1024 * 1024;

pub fn write(source: &str, message: &str) {
    let message = message.trim_end();
    eprintln!("[{source}] {message}");
    let _ = append(source, message);
}

pub fn gui(message: &str) {
    write("gui", message);
}

pub fn core(message: &str) {
    write("core", message);
}

pub fn read_text() -> String {
    fs::read_to_string(harbor_log_path()).unwrap_or_default()
}

pub fn merge_pretty(local: &str, remote: &str) -> String {
    let mut entries = parse_entries(local);
    if !remote.is_empty() {
        entries.extend(parse_entries(remote));
    }
    entries.sort_by_key(|(ms, _, _, index)| (*ms, *index));
    let mut lines = Vec::with_capacity(entries.len());
    for (ms, source, message, _) in entries {
        lines.push(format_line(ms, source.as_str(), message.as_str()));
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn append(source: &str, message: &str) -> Result<(), String> {
    let path = harbor_log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {} failed: {error}", parent.display()))?;
    }
    rotate_if_needed(&path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;
    let line = format!("{} {source} {message}\n", now_ms());
    file.write_all(line.as_bytes())
        .map_err(|error| format!("write {} failed: {error}", path.display()))
}

fn rotate_if_needed(path: &std::path::Path) -> Result<(), String> {
    let Ok(meta) = fs::metadata(path) else {
        return Ok(());
    };
    if meta.len() <= MAX_LOG_BYTES {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;
    let keep = KEEP_LOG_BYTES.min(meta.len());
    file.seek(SeekFrom::End(-(keep as i64)))
        .map_err(|error| error.to_string())?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)
        .map_err(|error| error.to_string())?;
    if let Some(index) = tail.iter().position(|byte| *byte == b'\n') {
        tail = tail[index + 1..].to_vec();
    }
    fs::write(path, tail).map_err(|error| format!("truncate {} failed: {error}", path.display()))
}

fn parse_entries(raw: &str) -> Vec<(u128, String, String, usize)> {
    let mut entries = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let Some((ms_raw, rest)) = line.split_once(' ') else {
            continue;
        };
        let Ok(ms) = ms_raw.parse::<u128>() else {
            continue;
        };
        let (source, message) = rest.split_once(' ').unwrap_or((rest, ""));
        entries.push((ms, source.to_string(), message.to_string(), index));
    }
    entries
}

fn format_line(ms: u128, source: &str, message: &str) -> String {
    let (year, month, day, hour, min, sec) = local_civil_time(ms);
    format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}.{:03} [{source}] {message}",
        ms % 1000
    )
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn local_civil_time(ms: u128) -> (i32, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000) as libc::time_t;
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if !fill_local_time(&secs, &mut tm) {
            return (1970, 1, 1, 0, 0, 0);
        }
        (
            tm.tm_year + 1900,
            (tm.tm_mon + 1) as u32,
            tm.tm_mday as u32,
            tm.tm_hour as u32,
            tm.tm_min as u32,
            tm.tm_sec as u32,
        )
    }
}

#[cfg(unix)]
unsafe fn fill_local_time(secs: &libc::time_t, tm: &mut libc::tm) -> bool {
    !libc::localtime_r(secs, tm).is_null()
}

#[cfg(windows)]
unsafe fn fill_local_time(secs: &libc::time_t, tm: &mut libc::tm) -> bool {
    libc::localtime_s(tm, secs) == 0
}

#[cfg(test)]
mod tests {
    use super::merge_pretty;

    #[test]
    fn merge_pretty_orders_gui_and_core() {
        let local = "100 gui hello\n300 gui later\n";
        let remote = "200 core mid\n";
        let merged = merge_pretty(local, remote);
        let lines: Vec<&str> = merged.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("[gui] hello"));
        assert!(lines[1].contains("[core] mid"));
        assert!(lines[2].contains("[gui] later"));
    }
}
