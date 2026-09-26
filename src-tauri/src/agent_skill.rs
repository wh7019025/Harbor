use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use harbor_common::settings::{config_dir, home_dir};
use harbor_common::version::APP_VERSION;

const BUNDLED_SKILL_FILES: &[(&str, &str)] = &[
    (
        "SKILL.md",
        include_str!("../resources/skills/harbor/SKILL.md"),
    ),
    (
        "agents/openai.yaml",
        include_str!("../resources/skills/harbor/agents/openai.yaml"),
    ),
    (
        "references/task-paths.md",
        include_str!("../resources/skills/harbor/references/task-paths.md"),
    ),
    (
        "references/task-workflow.md",
        include_str!("../resources/skills/harbor/references/task-workflow.md"),
    ),
    (
        "references/settings.md",
        include_str!("../resources/skills/harbor/references/settings.md"),
    ),
    (
        "references/web-api.md",
        include_str!("../resources/skills/harbor/references/web-api.md"),
    ),
    (
        "references/version.md",
        include_str!("../resources/skills/harbor/references/version.md"),
    ),
    (
        "references/task-yaml.md",
        include_str!("../resources/skills/harbor/references/task-yaml.md"),
    ),
    (
        "references/group-yaml.md",
        include_str!("../resources/skills/harbor/references/group-yaml.md"),
    ),
    (
        "references/safety.md",
        include_str!("../resources/skills/harbor/references/safety.md"),
    ),
];

#[derive(Clone, Debug, Serialize)]
pub struct AgentSkillInfo {
    pub skill_dir: String,
    pub files: Vec<String>,
    pub prompt: String,
}

pub fn skill_dir() -> PathBuf {
    home_dir().join(".agents").join("skills").join("harbor")
}

pub fn sync_agent_skill() -> Result<AgentSkillInfo, String> {
    let skill_dir = skill_dir();
    let harbor_home = config_dir();
    fs::create_dir_all(&harbor_home)
        .map_err(|error| format!("create {} failed: {error}", harbor_home.display()))?;
    fs::create_dir_all(&skill_dir)
        .map_err(|error| format!("create {} failed: {error}", skill_dir.display()))?;

    for (relative_path, content) in BUNDLED_SKILL_FILES {
        let path = skill_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {} failed: {error}", parent.display()))?;
        }
        write_text(&path, content)?;
    }
    write_text(
        &skill_dir.join("references/version.md"),
        &version_reference(),
    )?;
    write_version_json()?;
    remove_legacy_agent_assets()?;

    Ok(agent_skill_info())
}

fn write_version_json() -> Result<(), String> {
    let path = config_dir().join("version.json");
    let content = serde_json::to_string_pretty(&serde_json::json!({ "app": APP_VERSION }))
        .unwrap_or_else(|_| "{}".into());
    write_text(&path, &content)
}

fn version_reference() -> String {
    format!(
        r#"# Harbor 版本

| 名称 | 当前值 | 命令行 |
|------|--------|--------|
| 应用版本 | {app} | `harbor --version` |

机器可读：`~/.harbor/version.json`

## Task / Group YAML 中的 `version`

YAML 顶部的 `version` 是 Harbor 应用版本，与 `harbor --version` 相同。

- Harbor API 保存时会自动写入当前应用版本。
- Agent 直接编辑 YAML 时，仅在旧值与当前应用不一致时写入 `harbor --version` 的原样输出。
- 不要把版本当作修订号，不要自行递增 preview 或 rc 编号。
- 已是 `{app}` 时不要修改 `version`。

字段说明见 [task-yaml.md](task-yaml.md) 和 [group-yaml.md](group-yaml.md)。
"#,
        app = APP_VERSION
    )
}

fn remove_legacy_agent_assets() -> Result<(), String> {
    let harbor_home = config_dir();
    for directory in [harbor_home.join("agent_doc"), harbor_home.join("mcp")] {
        if directory.exists() {
            fs::remove_dir_all(&directory)
                .map_err(|error| format!("remove {} failed: {error}", directory.display()))?;
        }
    }
    let mcp_example = harbor_home.join("mcp.example.json");
    if mcp_example.exists() {
        fs::remove_file(&mcp_example)
            .map_err(|error| format!("remove {} failed: {error}", mcp_example.display()))?;
    }
    Ok(())
}

pub fn agent_skill_info() -> AgentSkillInfo {
    let skill_dir = skill_dir();
    AgentSkillInfo {
        files: list_skill_files(&skill_dir),
        skill_dir: skill_dir.display().to_string(),
        prompt: "Use $harbor to create, maintain, or operate this Harbor workflow.".into(),
    }
}

fn list_skill_files(directory: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_skill_files(directory, directory, &mut files);
    files.sort();
    files
}

fn collect_skill_files(root: &Path, directory: &Path, output: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_skill_files(root, &path, output);
        } else if file_type.is_file() {
            if let Ok(relative_path) = path.strip_prefix(root) {
                output.push(relative_path.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

fn write_text(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|error| format!("write {} failed: {error}", path.display()))
}
