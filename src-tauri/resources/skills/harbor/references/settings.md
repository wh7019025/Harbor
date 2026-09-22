# Workspace 与 Search Paths

配置文件路径：`~/.harbor/settings.json`

Search Paths 属于**当前 workspace**。Agent 可以直接维护当前 workspace 的 `search_paths`，无需指导用户操作界面。

Agent 只应修改 `current_workspace` 对应那一项的 `search_paths`；其他 Harbor 设置必须保留原值，包括 `workspaces` 里的 `mode` / `ssh` / `localhost_only`、其余项、`current_workspace` 和 `metrics_*`。

```json
{
  "current_workspace": "default",
  "workspaces": [
    {
      "id": "default",
      "name": "default",
      "mode": "local",
      "localhost_only": true,
      "search_paths": [
        "/home/user/projects/my-project"
      ]
    }
  ],
  "metrics_fast_ms": 1000,
  "metrics_slow_ms": 10000
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `current_workspace` | string | 当前 workspace 的 `id`。Agent 不要改这个字段 |
| `workspaces` | object[] | workspace 列表。每项含 `id`、`name`、`mode`、`search_paths`，remote 时还有 `ssh` |
| `workspaces[].mode` | `"local"` / `"remote"` | 默认 `local`。Agent 不要改 |
| `workspaces[].ssh` | object | remote workspace 的 SSH：`host`、`user`、`port`、`auth`（`key` / `sshpass`）、`identity_file`、`password`（sshpass 时保存）。Agent 不要改 |
| `workspaces[].localhost_only` | bool | 启动该 workspace 对应 harbor_core 时是否只监听 127.0.0.1。local 默认 `true`，remote 默认 `false`。Agent 不要改 |
| `workspaces[].search_paths` | string[] | 该 workspace 搜索项目配置的起点目录。路径相对于 **harbor_core 所在机器** |

文件不存在时，Harbor 使用内置默认值：一个 `id/name = default` 的 workspace，其 `search_paths` 为空。

每个 workspace 的日志独立保存在 **core 所在机器** 的 `~/.harbor/workspace/<id>/log/`。全机任务占用与进程注册保存在 `~/.harbor/runtime/run/`，按 UUID 跨 workspace 共享；因此切换 workspace 不会停止任务，但日志列表只显示当前 workspace 的日志。Task / Group YAML **不**放在这些目录，只来自当前 workspace 的 `search_paths`。

远端 Workspace 的内置终端由 GUI 管理：GUI 通过现有 SSH 配置启动远端 `ttyd`，并用
SSH Tunnel 映射到本机回环端口。远端端口会从 `29386–29486` 自动选择，避免残留进程
占用固定端口。它不是 Task，不应为此创建 YAML，也不需要加入
`search_paths`。Harbor 会校验并自动部署自带的静态 `ttyd` 到
`~/.harbor/tools/ttyd/<sha256>/`；不要要求用户在远端安装 `ttyd`，也不要把它手动绑定
到 `0.0.0.0`。

`harbor_core` 二进制按版本分开放在 **core 所在机器** 的 `~/.harbor/core/<version>/harbor_core`，但每台机器同一时间只能运行一个 `harbor_core`，不区分版本。切换 remote workspace 不会自动连接或部署；用户需要在 GUI 中显式连接。连接顺序是 SSH 验证、运行中 Core/API 检查、版本与租约检查、匹配 release 检查、必要时复制、最后唤醒。当前管理它的 GUI 持有短时访问租约；其他版本不得替换使用中的 core。仅能发现存活 PID 但 API 不可达时也拒绝自动替换。

本机 GUI 与本机 harbor_core 共用 `~/.harbor/settings.json`。直接改 search_paths 后需要重启 `harbor_core`，或改用 HTTP `POST /api/v1/workspaces/search-paths`。

## 发现规则

每个 Search Path 必须是已存在的目录。Harbor 从该目录向下最多 5 层查找：

- `{search_path}/**/harbor_taskcfg/tasks/`
- `{search_path}/**/harbor_taskcfg/groups/`

应填写项目根或工作区根，而不是 `harbor_taskcfg` 本身。默认优先填写当前项目根，避免使用 `/`、HOME 或包含大量无关仓库的宽泛目录。

路径支持绝对路径、`~/...`，以及相对 HOME 的路径。local / remote 都使用 **harbor_core 所在机器** 上的目录；GUI 通过 HTTP `/api/v1/paths/suggestions` 列举，不走 SSH。

## Agent 维护 pipeline

### 阶段一：读取与检查

1. 读取现有 `~/.harbor/settings.json`；文件不存在时以 Harbor 默认值为基础创建。
2. 找到 `current_workspace` 对应的 workspace 项。
3. 确认当前项目的绝对路径和 `harbor_taskcfg` 位置。
4. 检查该 workspace 的 `search_paths` 是否已经覆盖该项目，并确认 `harbor_taskcfg` 位于向下 5 层以内。
5. 已覆盖时不要重复添加。

### 阶段二：添加路径

1. 选择能够发现配置的最窄实用目录，通常是项目根。
2. 确认目录真实存在。
3. 将规范化后的路径追加到**当前 workspace** 的 `search_paths`，并按规范化路径去重。
4. 保留 `settings.json` 的全部未知字段和值，不要用文档示例覆盖整个文件。

### 阶段三：删除路径

仅在用户明确要求删除，或已确认路径失效且不会影响其他项目配置时移除。不要因为当前任务未使用某条 Search Path 就擅自删除。不要改其他 workspace 的 `search_paths`。

### 阶段四：验证与生效

1. 确认 JSON 可以解析，当前 workspace 的 `search_paths` 是字符串数组且没有重复项。
2. 确认目标路径下的 `harbor_taskcfg/tasks/` 或 `harbor_taskcfg/groups/` 能在 5 层范围内被发现。
3. 告知用户重新加载配置或重启 Harbor，使直接编辑的设置生效。

## 示例

当前项目为 `/home/user/projects/my-project`，配置位于：

```text
/home/user/projects/my-project/harbor_taskcfg/tasks/
```

推荐添加到当前 workspace 的 `search_paths`：

```json
"/home/user/projects/my-project"
```

不要添加：

```json
"/home/user/projects/my-project/harbor_taskcfg"
```
