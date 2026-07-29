# Harbor AgentHelp

给 Agent / 自动化助手用的操作说明。修改任务、搜索路径、日志时按本文约定执行。

先读本索引，再按需打开子文档。文档目录在 `~/.harbor/agent_doc/`（Harbor 每次启动会刷新）。

## 结构树

```text
agent_doc/
├── AgentDoc.md      ← 你在这里
├── windows.md
├── taskcard/
│   ├── paths.md
│   └── create.md
├── yaml/
│   ├── task.md
│   └── group.md
├── logs.md
├── tips.md
└── MCP.md
```

## 文档索引

| 路径 | 内容 |
|------|------|
| [windows.md](windows.md) | 各功能窗口用途 |
| [taskcard/paths.md](taskcard/paths.md) | TaskCard 目录约定、search path、research |
| [taskcard/create.md](taskcard/create.md) | 新建任务流程 |
| [yaml/task.md](yaml/task.md) | Task YAML 格式与规则 |
| [yaml/group.md](yaml/group.md) | Group YAML 格式与规则 |
| [logs.md](logs.md) | 运行日志与历史查看 |
| [tips.md](tips.md) | 给 Agent 的建议 |
| [MCP.md](MCP.md) | Cursor MCP 配置与 resource URI |
