---
title: Web API
permalink: /reference/web-api/
createTime: 2026/09/20 00:44:43
---
# Web API

Core 默认监听端口 `29385`，当前 API revision 为 **6**，基础路径为：

```text
http://<host>:29385/api/v1
```

::: warning
当前 API 没有鉴权。远端 Core 只能部署在可信局域网或受控网络中，禁止直接暴露到公网。
:::

Harbor GUI 会持有短时访问租约，用来协调唯一 Core 的版本管理。其他版本发现 Core 正在使用时只显示占用状态，不会自动重装；该租约不是安全认证。

## 状态与发现

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/health` | Core 版本、API revision、进程和监听状态。 |
| `GET` | `/access` | 查看 GUI 访问租约。 |
| `POST` | `/access/claim` | 获取或刷新 GUI 访问租约。 |
| `POST` | `/access/release` | 释放 GUI 访问租约。 |
| `GET` | `/snapshot` | 一次获取 Workspace、Task、Group 与状态快照。 |
| `POST` | `/discovery/refresh` | 重新扫描搜索路径。 |

## Task

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/tasks` | 列出已发现 Task。 |
| `GET` | `/tasks/running` | 列出正在运行的 Task。 |
| `GET` | `/tasks/status` | 查询单个 Task 状态。 |
| `POST` | `/tasks/start` | 启动 Task。 |
| `POST` | `/tasks/stop` | 停止 Task。 |
| `POST` | `/tasks/restart` | 重启 Task。 |
| `POST` | `/tasks/stop-all` | 停止全部 Task。 |
| `GET/POST/PUT/DELETE` | `/tasks/yaml` | 读取、新建、更新或删除 Task YAML。 |
| `GET` | `/tasks/template` | 获取 Task 模板。 |

## Group

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/groups` | 列出已发现 Group。 |
| `POST` | `/groups/start` | 启动 Group。 |
| `POST` | `/groups/stop` | 停止 Group。 |
| `GET/POST/PUT/DELETE` | `/groups/yaml` | 读取、新建、更新或删除 Group YAML。 |
| `GET` | `/groups/template` | 获取 Group 模板。 |

## 托管进程

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/processes` | 列出 Harbor 启动且仍存活的进程组与成员进程。 |
| `POST` | `/processes/stop` | 使用 Task UUID 终止一个托管进程组。 |

任务主进程退出后，只要原进程组仍有成员，记录就不会消失。GUI 的任务管理器会将这种运行单元标记为“残留进程”。

## 日志、Workspace 与路径

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/logs/tasks` | 列出任务历史日志。 |
| `GET` | `/logs/task` | 读取任务日志；传入 `offset` 时增量读取，`tail_lines` 可让首次读取直接定位到末尾指定行数。 |
| `GET` | `/logs/core` | 读取 Core 日志。 |
| `POST` | `/workspaces/switch` | 切换 Core 当前 Workspace。 |
| `GET/POST/DELETE` | `/workspaces/search-paths` | 列出、添加或删除当前 Workspace 搜索路径。 |
| `GET` | `/paths/suggestions` | 获取路径补全候选。 |
| `GET` | `/paths/config-base` | 获取配置基准路径。 |

## UUID

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/uuids/new` | 生成新 UUID。 |
| `GET` | `/uuids/conflicts` | 列出重复 UUID。 |
| `POST` | `/uuids/reset` | 为指定配置重置 UUID。 |

请求和响应均使用 JSON。精确字段应以对应版本的 Core 源码和 GUI 调用为准；客户端必须先检查 `/health` 返回的 API revision。
