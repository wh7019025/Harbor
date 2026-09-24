---
title: 程序界面
permalink: /guide/panels/
createTime: 2026/09/20 00:44:43
---
# 程序界面

普通程序不需要链接 Harbor SDK。只要 Task 能启动它、程序能输出日志，就已经可以由 Harbor 管理。

当程序还需要提供可视化、控制界面、实时视频或交互操作时，可以选择两种入口：

- `webview_interface`：程序主动提供 Web 页面，使用网页图标。
- `vnc_interface`：Harbor 把远端原生窗口放入共享虚拟桌面，使用立方体图标。

![Harbor 中运行的 Robot Panel](/images/harbor-app.png)

::: important 先判断是否有 UI
没有 UI 的 Task 不配置任何 interface。只有程序自己提供网页时使用
`webview_interface`；只有远端需要操作原生窗口时使用 `vnc_interface`。
:::

## VNC：远端操作原生窗口

如果程序已经有 Qt、GTK 或其他 X11 界面，不需要再开发网页。只需为 Task
声明 VNC 接口：

```yaml
vnc_interface:
  - panel_name: desktop
command:
  argv: [your-gui-program]
```

程序不需要读取 Harbor 环境变量，也不需要知道 VNC 如何配置。local workspace
仍直接打开程序的原生窗口，不启动 VNC；只有 remote workspace 会按需启动机器级
共享虚拟显示屏，并从固定端口 `23682` 发布 noVNC 页面。所有 VNC Task 的窗口都在
同一个桌面中；停止其中一个 Task 不会关闭桌面或其他窗口。WebView 不受影响。完整配置见
[VNC Interface 示例](https://github.com/wh7019025/Harbor/tree/main/examples/vnc_interface)。

运行 Task 的机器需要安装 TigerVNC、noVNC、websockify、Openbox 和 util-linux。远端
Core 还会提供独立的真实桌面入口：显示器图标镜像 `DISPLAY=:0`，立方体图标则打开
`vnc_interface` Task 所在的共享虚拟桌面。真实桌面额外需要
`tigervnc-scraping-server`，但不改变 Task 的运行位置。

## WebView：程序提供 Web 页面

Harbor 负责启动 Task、提供 WebView 入口并构造访问地址；程序负责运行 HTTP 服务和页面内容。

WebView 不是 Harbor 托管的前端文件，也不是一个独立 Task。它跟随所属 Task 一起启动和停止。

### 先运行完整示例

Harbor 仓库提供了可以直接运行的 [Robot Panel 完整示例](https://github.com/wh7019025/Harbor/tree/main/examples/webpage_view)。它不是静态页面截图，而是一条完整接入通路：

- Harbor Task 启动 Python 后端并注入 WebView 端口。
- Vue 页面通过 HTTP 获取机器人状态和发送控制命令。
- WebSocket 持续传输动态视频画面。
- 存在 ROS 2 环境时接入真实 Topic，否则自动使用 mock 数据。
- 默认窗口使用三栏布局，最小窗口切换为单视口功能页。

体验步骤：

1. 将 Harbor 仓库根目录或 `examples/webpage_view` 加入 Workspace 的 Search Paths。
2. 启动 Task `robot-panel`。
3. 点击 Task 行上的网页图标。

直接查看关键文件：

- [Task YAML](https://github.com/wh7019025/Harbor/blob/main/examples/webpage_view/harbor_taskcfg/tasks/robot-panel.yaml)
- [Vue Panel 页面](https://github.com/wh7019025/Harbor/blob/main/examples/webpage_view/src/App.vue)
- [Python / ROS 2 / WebSocket 后端](https://github.com/wh7019025/Harbor/blob/main/examples/webpage_view/robot_panel.py)
- [示例运行说明](https://github.com/wh7019025/Harbor/blob/main/examples/webpage_view/README.md)

## WebView 如何工作

```text
Harbor 启动 Task
       ↓ 注入名称、端口和监听范围
程序启动 HTTP / WebSocket 服务
       ↓
Harbor 在 Task 上显示 WebView 按钮
       ↓
用户在独立窗口中操作程序
```

一个 Task 可以声明一个或多个 WebView。常见用途包括：

- 机器人状态、传感器数据和告警展示。
- 模式切换、参数调整和任务控制。
- 相机视频、地图或三维可视化。
- ROS 2 Topic 观察与简单控制。
- 程序自己的调试或运维界面。

只需要启动和查看日志的程序，不必为了“接入 Harbor”额外开发界面。

## 第一步：声明 WebView

在 Task YAML 中添加 `webview_interface`：

```yaml
webview_interface:
  - panel_name: robot_panel
    interface_port: 23681
    localhost_only: false
```

`panel_name` 是 Harbor 中显示的面板名称，`interface_port` 是唯一的生产端口配置来源。

不要同时在 YAML `env`、程序源码、Vite 配置或启动脚本中再次固定同一个端口。

## 第二步：读取运行信息

Harbor 启动单 WebView Task 时注入：

- `HARBOR_WEBVIEW_NAME`
- `HARBOR_WEBVIEW_INTERFACE_PORT`
- `HARBOR_WEBVIEW_LOCALHOST_ONLY`

程序应从环境变量决定监听地址与端口：

```python
import os

port = int(os.environ["HARBOR_WEBVIEW_INTERFACE_PORT"])
localhost_only = os.environ["HARBOR_WEBVIEW_LOCALHOST_ONLY"] == "true"
host = "127.0.0.1" if localhost_only else "0.0.0.0"

app.run(host=host, port=port)
```

缺少或无法解析端口时，程序应明确退出并打印错误，不要静默回退到另一个端口。

多 WebView Task 使用带标准化名称的变量。例如 `robot-panel` 对应：

```text
HARBOR_WEBVIEW_ROBOT_PANEL_INTERFACE_PORT
HARBOR_WEBVIEW_ROBOT_PANEL_LOCALHOST_ONLY
```

## 第三步：提供页面

Task 启动后，程序需要在声明端口的 `/` 路径提供 HTTP 页面。Harbor 检测到 Task 运行后，会在任务行显示对应的网页图标。

Panel 窗口标题由 Harbor 提供，页面内容不要再次显示产品名、Task 名或 `Dashboard` 一类重复标题。首屏应直接用于状态、操作、告警和数据。

默认 Panel 窗口为 `1100 × 780`。页面应在一个视口内完成主要操作，只让日志、列表或表格等局部区域滚动；不要把桌面布局堆叠成长页面。

## HTTP、WebSocket 与视频

前端请求自己的后端时使用相对地址：

```javascript
const response = await fetch('/api/state')
```

WebSocket 应根据当前 Panel 地址构造，不能固定为 localhost：

```javascript
const scheme = location.protocol === 'https:' ? 'wss' : 'ws'
const socket = new WebSocket(`${scheme}://${location.host}/ws`)
```

视频可以通过 WebSocket、MJPEG 或其他 HTTP 通路传输。Harbor 不处理媒体内容，只负责打开 Panel；编码、帧率、重连和显示逻辑由程序实现。

## 接入 ROS 2

Panel 后端可以同时作为 ROS 2 node：

1. Task 启动前加载 ROS 2 和机器人 Workspace。
2. 后端订阅 Topic，将状态或图像通过 HTTP/WebSocket 发送给前端。
3. 前端操作通过 API 交给后端，再由后端发布控制 Topic。

```yaml
command:
  shell: bash
  script: |
    source /opt/ros/humble/setup.bash
    source install/setup.bash
    exec python3 -u robot_panel.py
```

控制真实机器人时，速度限制、急停链路和权限校验必须由机器人程序保证，不能只依赖网页按钮。

## 本地与远端


- 本地 WebView 通常使用 `localhost_only: true`，仅监听 `127.0.0.1`。
- 远端 WebView 需要从 GUI 所在机器访问时，使用 `localhost_only: false` 并监听 `0.0.0.0`。
- Harbor 复制 WebView URL 时，`localhost_only: true` 使用 `127.0.0.1`；`false` 使用运行该 Task 的机器默认路由 IP。
- 远端端口必须在网络和防火墙中可达。
- 远端页面中的 HTTP 与 WebSocket 地址应使用当前 `location.host`，不要写死远端 IP。

Harbor 不会为 Panel 自动增加鉴权。监听非 localhost 地址时，只应在可信网络中使用，或由程序自行实现访问控制。

## 让 Panel 适合 Harbor

- 把运行状态写到 stdout，把错误和诊断信息写到 stderr。
- 及时刷新输出，Python 推荐使用 `python3 -u`。
- 正确处理停止信号，释放相机、串口、共享内存和端口。
- Shell 启动脚本最后使用 `exec`，让 Panel 主进程直接接收停止信号。
- 使用 Harbor 的视觉规范和控件行为，不混用浏览器默认控件。
- 为无硬件环境提供 mock 数据，便于独立开发和演示。

## 按需复用示例

接入自己的程序时，可以直接从 [`examples/webpage_view`](https://github.com/wh7019025/Harbor/tree/main/examples/webpage_view) 复制最接近需求的部分：

- Vue 3 + Vite 前端。
- Python HTTP 与 WebSocket 后端。
- 动态视频通路。
- ROS 2 Topic 订阅和控制发布。
- 无 ROS 环境下自动使用 mock 数据。
- 直接复用 Harbor 的设计变量与交互风格。

不需要照搬整个示例。普通状态页可以只保留 HTTP；实时画面再增加 WebSocket；需要机器人通信时再接入 `Ros2Bridge`。

让 AI 为现有程序接入 WebView：

```txt
使用 $harbor，为当前程序添加 WebView。
```
