# Task YAML

完整示例（Agent 直接写**新**文件时，`version` 原样设为 `harbor --version` 的输出；修改已有文件且 version 已正确时不必改）：

```yaml
version: "0.1.4"
id: demo-ping
name: Demo Ping
description: 演示用的周期性 hello ping
workdir: $(harbor_taskcfg_dir)/..
env: {}
configs:
  - id: development
    name: 开发环境
    env:
      LOG_LEVEL: debug
  - id: production
    name: 生产环境
    env:
      LOG_LEVEL: info
      WORKERS: "4"
default_config: development
sudo: false
command:
  shell: sh
  script: |
    echo hello
    sleep 1
```

或 argv 形式：

```yaml
version: "0.1.4"
id: uname-kernel
name: Uname Kernel
description: ""
workdir: "~"
env: {}
sudo: false
command:
  argv:
    - uname
    - -a
```

## 字段

| 字段 | 必填 | 说明 |
|------|------|------|
| `version` | 是 | Harbor **应用**版本（`harbor --version` 的原样输出），不是配置修订号。**API 保存时自动维护**；Agent 直接编辑时仅在旧值或与应用不一致时更新，**禁止自行递增 rc 号**。详见 [version.md](../version.md) |
| `id` | 是 | 字母数字、`-`、`_`；同一 `tasks/` 目录内唯一 |
| `name` | 否 | 显示名；省略或空时 UI 显示 `id` |
| `description` | 否 | 任务说明（可为 `""`）。**Agent 编写时尽量用中文**，简要说明任务用途；无说明时可留空 |
| `workdir` | 是 | 启动时工作目录，不可为空。见下文 |
| `env` | 否 | 环境变量 map，默认 `{}` |
| `configs` | 否 | 有序的运行配置列表；每项可覆盖顶层 `env` |
| `configs[].id` | 是 | Task 内唯一，字符规则与 Task `id` 相同 |
| `configs[].name` | 否 | UI 显示名；省略或为空时显示 config `id` |
| `configs[].env` | 否 | 此 config 的环境变量，默认 `{}` |
| `default_config` | 否 | 默认 config `id`；省略时使用 `configs` 第一项 |
| `sudo` | 否 | 默认 `false`；为 `true` 时启动需输入密码 |
| `command` | 是 | 执行方式：`argv` **或** `shell` + `script` 二选一 |

## command

- **argv**：直接执行，第一项为可执行文件
- **shell + script**：等价于 `{shell} -lc {script}`

二者不可同时为空。

## configs

- `configs` 不存在或为空时，Task 行为与旧配置完全一致。
- UI 仅在一个 Task 有多个 config 时显示选择栏；没有 config 或只有一个时保持紧凑单行。
- 运行时先加载顶层 `env`，再用所选 `configs[].env` 覆盖同名变量。
- `default_config` 若填写，必须引用当前 Task 中已有的 config；未填写时选择列表第一项。
- 同一 Task 同时只能运行一个 config。切换选择后，下一次启动或重启使用新 config。
- Group 还可以通过 `tasks[].env` 再次覆盖 config 环境变量，完整顺序为：`task.env` → `config.env` → `group task env`。

## description

- **尽量用中文**写一句简短说明，描述任务做什么、在什么场景下用。
- 用户未提供且一时无法概括时可写 `""`，但不要用英文占位敷衍。
- `name` 可以是英文标识风格；`description` 面向人读，优先中文。

## workdir

- `"~"` 或 `"~/..."`：当前用户 HOME。必须加引号；YAML 会把未加引号的 `~` 解析为 `null`
- `null`：等价于 `$(harbor_taskcfg_dir)`，表示当前 Task 所属的 `harbor_taskcfg` 目录
- 绝对路径
- 相对路径：相对于**项目根**（该 YAML 所在 `harbor_taskcfg` 的**父目录**）
- `$(harbor_taskcfg_dir)`：当前 `harbor_taskcfg` 目录；项目任务常用 `$(harbor_taskcfg_dir)/..` 指向仓库根

## 运行时身份

- 跨工程允许同名 `id`；同一 `tasks/` 目录内短 id 仍唯一
- 运行时身份为 `(prefix_path, id)`

## 不要写入 YAML

`folder`、`prefix_path`、`taskcfg_dir` 由 Harbor 根据文件位置自动填充。
