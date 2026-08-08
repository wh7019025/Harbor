# Harbor 版本

| 名称 | 当前值 | 命令行 |
|------|--------|--------|
| 应用版本 | 0.1.2 | `harbor --version` |

机器可读：`~/.harbor/version.json`（Harbor 启动时更新）

## Task / Group YAML 中的 `version`

YAML 顶部的 `version` 是 **Harbor 应用版本**（与 `harbor --version` 相同，例如 `"0.1.2"`）。

### 谁维护 `version`？

| 修改方式 | `version` 如何处理 |
|----------|-------------------|
| **Harbor API 保存** | Harbor **自动**写入当前应用版本 |
| **Agent 直接编辑 YAML 文件** | Agent **须手动**将 `version` 设为 `harbor --version` 的输出 |

Agent 改文件前请先运行：

```bash
harbor --version
```

并将结果写入 YAML，例如 `version: "0.1.2"`（建议加引号）。

旧文件若仍为 `version: 1` 等历史值，Harbor 仍可加载；Agent 更新文件时应一并改为当前应用版本。

详见 [yaml/task.md](yaml/task.md)、[yaml/group.md](yaml/group.md)。
