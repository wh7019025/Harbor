# Harbor 版本


| 名称   | 当前值       | 命令行                |
| ---- | --------- | ------------------ |
| 应用版本 | 0.1.3 | `harbor --version` |


机器可读：`~/.harbor/version.json`（Harbor 启动时更新）

## Task / Group YAML 中的 `version`

YAML 顶部的 `version` 是 **Harbor 应用版本**（与 `harbor --version` 相同，例如 `"0.1.3"`）。

**不是** Task / Group 的修订号或「改一次加一」的版本计数。

### 谁维护 `version`？


| 修改方式                   | `version` 如何处理                            |
| ---------------------- | ----------------------------------------- |
| **Harbor API 保存**      | Harbor **自动**写入当前应用版本                     |
| **Agent 直接编辑 YAML 文件** | 仅在需要时设为 `harbor --version` 的**当前输出**（见下文） |


### Agent 编辑 YAML 时

1. **禁止自行递增版本号**（例如 `0.1.2-rc3` → `0.1.2-rc4`）。rc 号由 Harbor 维护者在**发布新版本**时修改，与 Task / Group 内容改动无关。
2. **原样复制** `harbor --version` 的输出，不要猜测、推算或 +1。
3. **修改已有文件时**：若 YAML 中 `version` 已与 `harbor --version` 一致，**不要改动** `version`，只改用户要求的内容。
4. **仅在以下情况更新** `version`：
  - 文件仍是旧格式（如 `version: 1`）
  - YAML 中的 `version` 与 `harbor --version` **确实不一致**

需要确认版本时运行：

```bash
harbor --version
```

并将结果写入 YAML，例如 `version: "0.1.3"`（建议加引号）。同一应用版本下无论改多少次 Task / Group，此值保持不变。

详见 [yaml/task.md](yaml/task.md)、[yaml/group.md](yaml/group.md)。