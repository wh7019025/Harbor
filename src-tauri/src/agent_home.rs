use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::settings::config_dir;
use crate::version::APP_VERSION;

const BUNDLED_DOCS: &[(&str, &str)] = &[
    (
        "AgentDoc.md",
        include_str!("../resources/agent_doc/AgentDoc.md"),
    ),
    (
        "taskcard/paths.md",
        include_str!("../resources/agent_doc/taskcard/paths.md"),
    ),
    (
        "taskcard/create.md",
        include_str!("../resources/agent_doc/taskcard/create.md"),
    ),
    (
        "settings.md",
        include_str!("../resources/agent_doc/settings.md"),
    ),
    (
        "version.md",
        include_str!("../resources/agent_doc/version.md"),
    ),
    (
        "yaml/task.md",
        include_str!("../resources/agent_doc/yaml/task.md"),
    ),
    (
        "yaml/group.md",
        include_str!("../resources/agent_doc/yaml/group.md"),
    ),
    ("tips.md", include_str!("../resources/agent_doc/tips.md")),
];

const RETIRED_AGENT_DOCS: &[&str] = &["windows.md", "logs.md", "MCP.md"];

const BUNDLED_MCP_SERVER: &str = include_str!("../resources/mcp/index.mjs");

#[derive(Clone, Debug, Serialize)]
pub struct AgentHelpInfo {
    pub home: String,
    pub agent_doc_dir: String,
    pub files: Vec<String>,
    pub prompt: String,
    pub mcp_example: String,
}

pub fn harbor_home() -> PathBuf {
    config_dir()
}

pub fn agent_doc_dir() -> PathBuf {
    harbor_home().join("agent_doc")
}

pub fn sync_agent_doc() -> Result<AgentHelpInfo, String> {
    let home = harbor_home();
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
    for rel in RETIRED_AGENT_DOCS {
        let path = doc_dir.join(rel);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("remove retired doc {} failed: {e}", path.display()))?;
        }
    }
    write_text(&mcp_dir.join("index.mjs"), BUNDLED_MCP_SERVER)?;
    write_text(
        &home.join("mcp.example.json"),
        &mcp_example_json(&mcp_dir.join("index.mjs")),
    )?;
    write_version_files(&home, &doc_dir)?;

    Ok(agent_help_info())
}

fn write_version_files(home: &Path, doc_dir: &Path) -> Result<(), String> {
    let version_json = serde_json::json!({ "app": APP_VERSION });
    write_text(
        &home.join("version.json"),
        &serde_json::to_string_pretty(&version_json).unwrap_or_else(|_| "{}".into()),
    )?;
    write_text(&doc_dir.join("version.md"), &version_doc())?;
    Ok(())
}

fn version_doc() -> String {
    format!(
        r#"# Harbor 版本

| 名称 | 当前值 | 命令行 |
|------|--------|--------|
| 应用版本 | {app} | `harbor --version` |

机器可读：`~/.harbor/version.json`

## Task / Group YAML 中的 `version`

YAML 顶部的 `version` 是 **Harbor 应用版本**（与 `harbor --version` 相同，例如 `{app}`）。

| 修改方式 | `version` 如何处理 |
|----------|-------------------|
| Harbor API 保存 | Harbor **自动**写入当前应用版本 |
| Agent 直接编辑 YAML 文件 | Agent **须手动**设为 `harbor --version` 的输出 |

- **版本来源**：仅 `harbor --version`，勿自行编造
- **旧文件**：历史上 `version: 1` 等仍可加载；Agent 更新时应改为当前 `{app}`

字段说明见 [yaml/task.md](yaml/task.md)、[yaml/group.md](yaml/group.md)。
"#,
        app = APP_VERSION
    )
}

pub fn agent_help_info() -> AgentHelpInfo {
    let home = harbor_home();
    let doc_dir = agent_doc_dir();
    let files = list_doc_files(&doc_dir);
    let mcp_script = home.join("mcp").join("index.mjs");
    let prompt = format!(
        "请先阅读 Harbor Task / Group 配置手册：{}\n\
优先打开 AgentDoc.md，再按需读取 yaml/task.md、yaml/group.md 和 taskcard/paths.md。\n\
这里只包含创建和维护 Task / Group 配置所需的知识；界面和其他产品功能不需要关注。\n\
Agent 直接修改 YAML 时，须将 version 设为 `harbor --version` 的输出；详见 version.md。\n\
Task / Group 的 description 尽量用中文简要说明用途。",
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
            "harbor": {
                "command": "node",
                "args": [script.display().to_string()]
            }
        }
    }))
    .unwrap_or_else(|_| "{}".into())
}
