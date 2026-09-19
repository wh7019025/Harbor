---
title: 接入 Web Panel
permalink: /integration/web-panel/
createTime: 2026/09/20 00:21:48
---
# 接入 Web Panel

程序提供 HTTP 控制页时，可以声明 `panel_interface`，让 Harbor 在 Task 行中显示面板入口。

```yaml
panel_interface:
  - panel_name: robot_panel
    interface_port: 23681
    localhost_only: false
```

## 从环境变量读取配置

Harbor 启动单面板 Task 时注入：

- `HARBOR_PANEL_NAME`
- `HARBOR_PANEL_INTERFACE_PORT`
- `HARBOR_PANEL_LOCALHOST_ONLY`

Python 示例：

```python
import os

port = int(os.environ["HARBOR_PANEL_INTERFACE_PORT"])
localhost_only = os.environ["HARBOR_PANEL_LOCALHOST_ONLY"] == "true"
host = "127.0.0.1" if localhost_only else "0.0.0.0"

app.run(host=host, port=port)
```

不要在程序源码中再次固定端口。多面板 Task 应读取带标准化面板名的环境变量。

## WebSocket 与视频

WebSocket、MJPEG 或其他实时通路都由程序自己的 HTTP 服务提供，Harbor 只负责打开面板地址。前端应根据当前页面 host 构造 WebSocket URL，避免把远端地址写死为 localhost。

```javascript
const scheme = location.protocol === 'https:' ? 'wss' : 'ws'
const socket = new WebSocket(`${scheme}://${location.host}/ws`)
```

## 网络边界

- 本地面板可设置 `localhost_only: true`。
- 远端直接访问需要监听 `0.0.0.0`，并确保端口从 GUI 所在机器可达。
- Harbor 不会自动给面板增加鉴权，对外监听时由程序自行保护。

完整示例位于 Harbor 仓库的 `examples/webpage_view`，包含动态视频、WebSocket、ROS 2 和 mock 数据。
