# Harbor Web API

Harbor 运行期间在 **17890** 端口提供无鉴权 HTTP 接口。关闭 Harbor 后接口一并消失。

默认只监听 `127.0.0.1`。设置页可关闭 **localhost only**，改为监听 `0.0.0.0`（局域网可访问、无鉴权，可起停任务）。

基础地址：

- localhost only：`http://127.0.0.1:17890`
- 局域网：`http://0.0.0.0:17890`

约定：GET 用 query，POST 用 JSON。成功 `200` + JSON；失败 `4xx` + `{ "error": "..." }`。

Task / Group 主键是 `(prefix_path, id)`。`id` 必填；`prefix_path` 可省略，仅当该 id 全局唯一时生效，否则返回冲突错误。

start / restart 可带 `config_id`、`sudo_password`。密码不会写入日志。

## GET

| 路径 | 说明 |
|------|------|
| `/get/health` | `{ "ok": true, "version": "..." }` |
| `/get/task/lists` | `{ "tasks": [...] }` |
| `/get/running_process` | 当前运行中的 task |
| `/get/group/lists` | `{ "groups": [...] }` |
| `/get/task/status?id=&prefix_path=` | 单个 task |
| `/get/task/log_info?id=&prefix_path=` | 日志列表，可按 task id 过滤 |
| `/get/task/log?file=` 或 `?id=`，可选 `offset` | 读日志；带 `offset` 时返回 chunk（`next_offset` / `reset`） |

## POST

| 路径 | Body |
|------|------|
| `/set/task/start` | `{ "id", "prefix_path?", "config_id?", "sudo_password?" }` |
| `/set/task/stop` | `{ "id", "prefix_path?" }` |
| `/set/task/restart` | 同 start |
| `/set/task/stop_all` | 无 body |
| `/set/group/start` | `{ "id", "prefix_path?", "sudo_password?" }` |
| `/set/group/stop` | `{ "id", "prefix_path?" }` |
| `/set/research` | 无 body，重新扫描 search paths |

示例：

```bash
curl http://127.0.0.1:17890/get/health
curl http://127.0.0.1:17890/get/task/lists
curl -X POST http://127.0.0.1:17890/set/task/start \
  -H 'Content-Type: application/json' \
  -d '{"id":"demo","config_id":"production"}'
```
