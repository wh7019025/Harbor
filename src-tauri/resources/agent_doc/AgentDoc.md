# Harbor Task / Group 配置手册

本文档只帮助 Agent 创建和维护 Harbor 的 Task、Group 配置。界面布局、按钮位置、运行监控等其他产品功能不属于 Agent 需要了解的范围。

## Agent 工作流

1. 运行 `harbor --version`，取得 YAML 要写入的 `version`。
2. 确认配置属于当前项目还是全局环境，按 [taskcard/paths.md](taskcard/paths.md) 选择目录。
3. 按 [taskcard/create.md](taskcard/create.md) 直接创建文件，并遵循 [yaml/task.md](yaml/task.md) 的格式。
4. 需要组合任务时，按 [yaml/group.md](yaml/group.md) 创建或修改 Group。
5. 若项目配置尚未被发现，只修改 [settings.md](settings.md) 中与配置发现有关的字段。
6. 检查 YAML、Task 引用、路径和命令；不要写入 Harbor 自动生成的字段。

文档发布到 `~/.harbor/agent_doc/`，Harbor 启动时会刷新。

## 文档索引

| 路径 | 内容 |
|------|------|
| [version.md](version.md) | 获取并维护 YAML `version` |
| [settings.md](settings.md) | 配置根目录和项目发现路径 |
| [taskcard/paths.md](taskcard/paths.md) | Task / Group 的存放与发现规则 |
| [taskcard/create.md](taskcard/create.md) | Agent 直接创建 Task / Group 的流程 |
| [yaml/task.md](yaml/task.md) | Task YAML 格式与规则 |
| [yaml/group.md](yaml/group.md) | Group YAML 格式与规则 |
| [tips.md](tips.md) | 修改配置时的安全检查 |
