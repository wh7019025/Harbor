---
title: Harbor Skill
permalink: /integration/agent-skill/
createTime: 2026/09/20 01:30:00
---
# Harbor Skill

Harbor 启动时会把内置的 `harbor` Skill 安装到：

```text
~/.agents/skills/harbor/
├── SKILL.md
├── agents/openai.yaml
└── references/
```

支持 Agent Skills 的工具可以自动发现它。首次安装或 Harbor 更新后，新建 Agent 会话即可加载新版本。

## 使用方式

可以明确调用：

```text
Use $harbor to create a Task for this program.
```

也可以直接提出与 Harbor 有关的需求，例如创建 Task、编排 Group、维护 Workspace Search Paths、启动任务或读取日志。Skill 的描述会帮助 Agent 判断何时加载。

## Skill 包含什么

- 创建和维护 Task / Group 的工作流。
- Task YAML 与 Group YAML 字段参考。
- Workspace、项目发现和 `search_paths` 规则。
- UUID 与 Harbor 版本规则。
- `harbor_core` HTTP API、运行状态和日志操作。
- 修改运行中任务前应遵守的安全检查。

## 为什么不再使用 Harbor MCP

旧 MCP 只把静态 `agent_doc` 文件暴露为 Resources，没有提供额外的运行工具。Skill 可以直接携带同一批工作流和参考资料，并按需加载；运行操作则直接调用 `harbor_core` HTTP API。因此不再需要单独配置 Harbor MCP。

Harbor 会在迁移时清理旧的 `~/.harbor/agent_doc/`、`~/.harbor/mcp/` 和 `~/.harbor/mcp.example.json`。
