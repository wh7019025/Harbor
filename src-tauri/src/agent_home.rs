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
        "web_api.md",
        include_str!("../resources/agent_doc/web_api.md"),
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
| Agent 直接编辑 YAML 文件 | 仅在旧值或与当前应用不一致时，设为 `harbor --version` 的**原样输出** |

- **不是修订号**：YAML `version` 是应用版本，不是「改一次加一」；**禁止自行递增 rc 号**（如 rc3→rc4）
- **版本来源**：仅 `harbor --version`，勿猜测或 +1
- **已有文件**：若已是 `{app}` 则修改内容时**不要动** `version`
- **旧文件**：历史上 `version: 1` 等仍可加载；应改为当前 `{app}`

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
        "Harbor 使用规范\n\
\n\
1. Harbor 是什么\n\
Harbor 是本机任务控制台：用 YAML 定义 Task / Group，在桌面里启动、停止、看日志。Harbor 进程在跑时，也提供无鉴权 HTTP 接口（默认 http://127.0.0.1:17890）。\n\
\n\
2. Harbor 提供什么功能\n\
- Task：一条可运行命令；可用 configs 覆盖 env，同一 Task 同时只能跑一份\n\
- Group：按顺序拉起多条 Task，可指定 config 和额外 env\n\
- 发现：全局 ~/.harbor/harbor_taskcfg，以及 search_paths 下最多 5 层的项目 harbor_taskcfg\n\
- Web API：列表、起停、日志；Harbor 关闭后接口消失\n\
\n\
3. Harbor 的文档如何阅读\n\
文档目录：{}\n\
先读 AgentDoc.md（索引和工作流），再按需打开：\n\
- yaml/task.md、yaml/group.md：YAML 格式\n\
- taskcard/paths.md、taskcard/create.md：存放位置和创建流程\n\
- settings.md：search_paths / taskcard_root\n\
- version.md：YAML version 规则\n\
- web_api.md：HTTP 接口\n\
- tips.md：修改时的安全检查\n\
\n\
4. Agent 应该关注什么\n\
只负责创建和维护 Task / Group YAML，以及必要时直接改 ~/.harbor/settings.json 里的 search_paths。不要管界面布局、按钮、监控面板。项目配置写在仓库的 harbor_taskcfg/；用户没要求 Group 就不要编 Group。\n\
用户可能希望 Agent 控制 Harbor 的运行行为（列表、起停、看日志等）。Harbor 开着时，Agent 可灵活调用 Web API（见 web_api.md）完成这些需求，不必指挥用户点界面。\n\
\n\
注意事项\n\
- YAML version 必须原样等于 `harbor --version`。禁止自行递增 rc 号；已是当前应用版本时不要改 version。详见 version.md。\n\
- description 尽量用中文说明用途。\n\
- 定位 Task 用 (prefix_path, id)；同名 id 可跨项目存在。\n\
- 不要改正在运行的 Task 定义，先停再改。\n\
- 未要求变更的字段、命令、环境变量一律保留。",
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
