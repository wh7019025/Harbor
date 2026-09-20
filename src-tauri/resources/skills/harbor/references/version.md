# Harbor 版本


| 名称   | 当前值       | 命令行                |
| ---- | --------- | ------------------ |
| 应用版本 | 由 Git Tag 生成 | `harbor --version` |


机器可读：`~/.harbor/version.json`（Harbor 启动时更新）

Harbor GUI 与 `harbor_core` 的应用版本和 API revision **必须对应**。二进制按版本分开放在 `~/.harbor/core/<version>/harbor_core`（远端同样路径），但每台机器同一时间只允许运行一个 `harbor_core`，不区分版本。当前访问该机器的 Harbor GUI 占用这个唯一 core；发现版本或 API revision 不对应时，会关闭旧 core 并启动与当前 GUI 对应的 core，不会并行启动第二个。

发布版本来自精确的 `v*` Git Tag，例如 `v0.2.0-preview` 对应应用版本 `0.2.0-preview`。Tag 后的开发构建会附加提交数量和短 SHA；Agent 仍应始终读取 `harbor --version`，不能从 Git 历史自行推测版本。

GUI 只执行 `.harbor` 中的托管副本。构建 pipeline 先生成 release `harbor_core` 和 `harbor_core.sha256`，再把 SHA-256 固化进 GUI；安装和远端部署后必须再次校验哈希。版本相同但内容不同的 core 也会被替换，debug core 不得进入 `.harbor`。

## Task / Group YAML 中的 `version`

YAML 顶部的 `version` 是 **Harbor 应用版本**（与 `harbor --version` 相同，例如 `"0.2.0-preview"`）。

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

并将结果写入 YAML，例如 `version: "0.2.0-preview"`（建议加引号）。同一应用版本下无论改多少次 Task / Group，此值保持不变。

详见 [task-yaml.md](task-yaml.md)、[group-yaml.md](group-yaml.md)。
