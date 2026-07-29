use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::settings::home_dir;

const BUNDLED_DOCS: &[(&str, &str)] = &[
    (
        "AgentDoc.md",
        include_str!("../resources/agent_doc/AgentDoc.md"),
    ),
    ("windows.md", include_str!("../resources/agent_doc/windows.md")),
    (
        "taskcard/paths.md",
        include_str!("../resources/agent_doc/taskcard/paths.md"),
    ),
    (
        "taskcard/create.md",
        include_str!("../resources/agent_doc/taskcard/create.md"),
    ),
    (
        "yaml/task.md",
        include_str!("../resources/agent_doc/yaml/task.md"),
    ),
    (
        "yaml/group.md",
        include_str!("../resources/agent_doc/yaml/group.md"),
    ),
    ("logs.md", include_str!("../resources/agent_doc/logs.md")),
    ("tips.md", include_str!("../resources/agent_doc/tips.md")),
    ("MCP.md", include_str!("../resources/agent_doc/MCP.md")),
];

const BUNDLED_MCP_SERVER: &str = include_str!("../resources/mcp/index.mjs");

#[derive(Clone, Debug, Serialize)]
pub struct AgentHelpInfo {
    pub home: String,
    pub agent_doc_dir: String,
    pub files: Vec<String>,
    pub prompt: String,
    pub mcp_example: String,
}

pub fn superterm_home() -> PathBuf {
    home_dir().join(".superterm")
}

pub fn agent_doc_dir() -> PathBuf {
    superterm_home().join("agent_doc")
}

pub fn sync_agent_doc() -> Result<AgentHelpInfo, String> {
    let home = superterm_home();
    let doc_dir = agent_doc_dir();
    let mcp_dir = home.join("mcp");
    fs::create_dir_all(&doc_dir).map_err(|e| format!("create {} failed: {e}", doc_dir.display()))?;
    fs::create_dir_all(&mcp_dir).map_err(|e| format!("create {} failed: {e}", mcp_dir.display()))?;

    for (rel, content) in BUNDLED_DOCS {
        let path = doc_dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {} failed: {e}", parent.display()))?;
        }
        write_text(&path, content)?;
    }
    write_text(&mcp_dir.join("index.mjs"), BUNDLED_MCP_SERVER)?;
    write_text(
        &home.join("mcp.example.json"),
        &mcp_example_json(&mcp_dir.join("index.mjs")),
    )?;

    Ok(agent_help_info())
}

pub fn agent_help_info() -> AgentHelpInfo {
    let home = superterm_home();
    let doc_dir = agent_doc_dir();
    let files = list_doc_files(&doc_dir);
    let mcp_script = home.join("mcp").join("index.mjs");
    let prompt = format!(
        "请先阅读 SuperTerm Agent 文档目录：{}\n\
优先打开 AgentDoc.md（结构树索引），再按需打开子文档（如 yaml/task.md、taskcard/paths.md）。\n\
该目录在 SuperTerm 每次启动时会自动更新。\n\
若已配置 SuperTerm MCP，可通过 resources 读取 superterm://agent_doc/<相对路径> 。",
        doc_dir.display()
    );
    AgentHelpInfo {
        home: home.display().to_string(),
        agent_doc_dir: doc_dir.display().to_string(),
        files,
        prompt,
        mcp_example: mcp_example_json(&mcp_script),
    }
}

fn list_doc_files(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_doc_files(dir, dir, &mut files);
    files.sort();
    files
}

fn collect_doc_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_doc_files(root, &path, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn write_text(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("write {} failed: {e}", path.display()))
}

fn mcp_example_json(script: &Path) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "superterm": {
                "command": "node",
                "args": [script.display().to_string()]
            }
        }
    }))
    .unwrap_or_else(|_| "{}".into())
}
