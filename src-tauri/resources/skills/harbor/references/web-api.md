# Harbor Web API

`harbor_core` 在 **29385** 端口提供无需登录或令牌的 HTTP 接口。Harbor GUI 关闭后 **core 继续运行**，接口也还在。每台机器同一时间只允许一个 `harbor_core` 进程运行，不区分版本。

Harbor GUI 只通过该 HTTP API 与 core 交互（含路径列举），并且只执行 `.harbor/core/<version>/harbor_core` 中通过 SHA-256 校验的 release 托管副本。SSH 只用来把 **与 GUI 固化哈希一致** 的 release `harbor_core` 及其动态链接器/依赖库复制到远端 `.harbor` 后启动。所有响应带 `X-Harbor-Version` 和 `X-Harbor-Api-Revision`。GUI 通过短时访问租约协调 core 生命周期；版本不符但仍被其他 GUI 使用时，不会自动关闭或重装。

启动参数 `--localhost-only`：

- `true`（本机默认）：只监听 `127.0.0.1`
- `false`（remote 默认）：监听 `0.0.0.0`；局域网内任何能连接该地址的设备都可调用 API，包括起停任务

远端 glibc 往往比 GUI 机器旧。SSH 复制时会带上本机的 `ld-linux` 与依赖库，用它们启动 `harbor_core`，避免 `GLIBC_2.xx not found`；复制完成后会在远端计算 SHA-256，校验通过才启动。

基础地址：

- localhost only：`http://127.0.0.1:29385`
- 局域网：`http://<host>:29385`

## 通用契约

- 当前 `api_revision`：`19`。
- GET 参数放 query；POST 请求使用 `Content-Type: application/json`。
- 所有响应都带 `X-Harbor-Version` 与 `X-Harbor-Api-Revision`。
- 成功通常返回 `200` JSON。动作成功统一包含 `{ "ok": true }`；列表响应使用具名数组字段。
- 失败统一返回 `{ "error": "说明" }`：参数错误 `400`、资源不存在 `404`、重复/运行中/引用冲突 `409`。
- 所有业务接口统一使用 `/api/v1` 前缀；无旧路径兼容层。

## 身份与 workspace

- API 在当前 workspace 的发现范围内用 `(prefix_path, id)` 查找 YAML。`prefix_path` 可省略，但同名 id 不唯一时返回 `409`。
- 任务运行占用使用 YAML `uuid`，同一台机器同一 UUID 同时只能运行一个实例。
- workspace 各自维护 `search_paths`；本地日志位于 `~/.harbor/workspace/<id>/log/`，远端机器统一使用 `~/.harbor/remote/log/`。
- `POST /api/v1/workspaces/switch` 只切换发现范围和日志视图，不停止任务；重新发现相同 UUID 时共享实时运行状态。
- start / restart 可带 `config_id`、`sudo_password`，密码不写入日志或响应。

## Core 与快照

| 方法 | 路径 | 请求 | 成功响应 |
|------|------|------|----------|
| GET | `/api/v1/health` | - | `{ "ok", "version", "api_revision", "workspace_id", "localhost_only", "pid" }` |
| GET | `/api/v1/access` | - | 当前 GUI 访问租约状态 |
| POST | `/api/v1/access/claim` | `{ "client_id", "gui_version" }` | 获取或刷新租约；其他 GUI 已占用时返回 `409` |
| POST | `/api/v1/access/release` | `{ "client_id", "gui_version" }` | 主动释放自己的租约 |
| GET | `/api/v1/snapshot` | - | `TaskCardSnapshot`：当前 workspace 的 paths、tasks、groups、UUID 冲突与错误 |
| GET | `/api/v1/metrics` | - | Core 所在机器的完整 CPU、内存、磁盘、网络与 GPU 指标 |
| GET | `/api/v1/metrics/performance` | - | CPU、网络与 GPU 性能指标 |
| GET | `/api/v1/metrics/resources` | - | 内存、Swap 与根磁盘资源指标 |
| GET | `/api/v1/services` | - | Core 托管的 Virtual VNC、Physical VNC 与 ttyd 状态；只读，不可单独停止 |
| POST | `/api/v1/discovery/refresh` | `{}` | `ResearchResult`：重新扫描后的目录与 search paths |
| POST | `/api/v1/workspaces/switch` | `{ "id" }` | `{ "ok": true, "workspace_id" }` |
| POST | `/api/v1/terminal/ensure` | `{ "workdir", "title" }` | `{ "ready": true, "port": 29386 }`；仅 remote runtime 可用 |

## Task 与 Group

| 方法 | 路径 | 请求 | 成功响应 |
|------|------|------|----------|
| GET | `/api/v1/tasks` | - | `{ "tasks": TaskSummary[] }` |
| GET | `/api/v1/tasks/running` | - | `{ "tasks": TaskSummary[] }`，仅当前发现范围内运行中的 Task |
| GET | `/api/v1/tasks/status` | `?id=&prefix_path?=` | 单个 `TaskSummary` |
| POST | `/api/v1/tasks/start` | `{ "id", "prefix_path?", "config_id?", "sudo_password?" }` | `{ "ok": true }` |
| POST | `/api/v1/tasks/stop` | `{ "id", "prefix_path?" }` | `{ "ok": true }` |
| POST | `/api/v1/tasks/restart` | 同 start | `{ "ok": true }` |
| POST | `/api/v1/tasks/stop-all` | `{}` | `{ "errors": string[] }`；逐个停止，返回失败项 |
| GET | `/api/v1/processes` | - | Harbor 启动且仍存活的进程组、PID、程序名与命令 |
| POST | `/api/v1/processes/stop` | `{ "uuid" }` | 终止指定 Task UUID 的整个托管进程组 |
| GET | `/api/v1/groups` | - | `{ "groups": GroupDefinition[] }` |
| POST | `/api/v1/groups/start` | `{ "id", "prefix_path?", "sudo_password?" }` | `{ "ok": true }` |
| POST | `/api/v1/groups/stop` | `{ "id", "prefix_path?" }` | `{ "ok": true }` |

## YAML 配置

| 方法 | 路径 | 请求 | 成功响应 |
|------|------|------|----------|
| GET | `/api/v1/tasks/yaml` | `?id=&prefix_path?=` | `{ "content", "folder" }` |
| POST | `/api/v1/tasks/yaml` | `{ "content", "folder?" }` | `{ "id" }` |
| PUT | `/api/v1/tasks/yaml` | `{ "id", "prefix_path?", "content", "folder?" }` | `{ "ok": true }` |
| DELETE | `/api/v1/tasks/yaml` | `{ "id", "prefix_path?" }` | `{ "ok": true }` |
| GET | `/api/v1/tasks/template` | - | `{ "content" }`，包含新 UUID |
| GET | `/api/v1/groups/yaml` | `?id=&prefix_path?=` | `{ "content", "folder" }` |
| POST | `/api/v1/groups/yaml` | `{ "content", "folder?" }` | `{ "id" }` |
| PUT | `/api/v1/groups/yaml` | `{ "id", "prefix_path?", "content", "folder?" }` | `{ "ok": true }` |
| DELETE | `/api/v1/groups/yaml` | `{ "id", "prefix_path?" }` | `{ "ok": true }` |
| GET | `/api/v1/groups/template` | - | `{ "content" }`，包含新 UUID |
| GET | `/api/v1/uuids/new` | - | `{ "uuid" }` |
| GET | `/api/v1/uuids/conflicts` | - | `{ "conflicts": [{ "uuid", "definitions" }] }` |
| POST | `/api/v1/uuids/reset` | `{ "path" }` | `{ "ok": true, "uuid" }` |

UUID 缺失由扫描过程自动生成并写回。UUID 重复时相关 Task/Group 拒绝启动；`path` 必须是当前已发现的 YAML 文件。

## 日志

| 方法 | 路径 | 请求 | 成功响应 |
|------|------|------|----------|
| GET | `/api/v1/logs/tasks` | `?id?=&prefix_path?=` | `{ "logs": TaskLogSummary[] }`，仅当前 workspace |
| GET | `/api/v1/logs/task` | `?file=` 或 `?id=&prefix_path?=` | `{ "file", "content", "truncated" }` |
| GET | `/api/v1/logs/task` | 上述参数加 `&offset=` | `{ "file", "content", "next_offset", "reset" }` |
| GET | `/api/v1/logs/core` | - | `{ "content" }`，core 本机 `~/.harbor/log/harbor.log` |

`file` 只能是日志列表返回的安全文件名。按 `id` 读取时要求该 Task 当前正在运行并且当前 workspace 可见。

## 路径

| 方法 | 路径 | 请求 | 成功响应 |
|------|------|------|----------|
| GET | `/api/v1/workspaces/search-paths` | - | `{ "search_paths": string[] }` |
| POST | `/api/v1/workspaces/search-paths` | `{ "path" }` | `{ "search_paths": string[] }` |
| DELETE | `/api/v1/workspaces/search-paths` | `{ "path" }` | `{ "search_paths": string[] }` |
| GET | `/api/v1/paths/suggestions` | `?prefix?=` | `{ "query": string, "paths": string[] }`，展开 `~` 后列 core 所在机器目录，最多 50 项 |
| GET | `/api/v1/paths/config-base` | `?prefix_path?=` | `{ "path" }` |

示例：

```bash
curl http://127.0.0.1:29385/api/v1/health
curl http://127.0.0.1:29385/api/v1/tasks
curl -X POST http://127.0.0.1:29385/api/v1/tasks/start \
  -H 'Content-Type: application/json' \
  -d '{"id":"demo","config_id":"production"}'
```
