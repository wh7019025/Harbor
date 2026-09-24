---
title: 远端运行
permalink: /guide/remote/
createTime: 2026/09/20 00:44:43
---
# 远端运行

远端 Workspace 让你在当前电脑的 Harbor 中，直接管理机器人或服务器上的 Task。

项目文件和程序仍然留在远端机器上。首次打开远端 Workspace 时，点击 Workspace 旁的连接按钮建立连接；任务发现、启动、停止和日志读取都由远端 Core 执行。切换到其他 Workspace 不会断开已经建立的连接。

::: important 核心规则
远端 Workspace 不是把本地任务“发送过去运行”。它连接的是远端机器上已经存在的项目和 `harbor_taskcfg`。
:::

## 开始前确认

先确保当前电脑可以正常 SSH 登录远端机器：

```bash
ssh <user>@<host>
```

还需要满足：

- 远端用户可以运行项目所需的命令和依赖。
- 当前电脑可以访问远端 SSH 端口。
- 当前电脑可以访问远端 TCP `29385`。
- 使用 `webview_interface` 时，对应 `interface_port` 需要可达。

如果这些网络路径被防火墙、容器或路由隔离，Harbor 无法替代底层网络配置。

## 创建远端 Workspace

1. 点击 Workspace 旁的 `+`。
2. 输入 Workspace 名称，模式选择 **remote**。
3. 填写 SSH host、user 和 port。
4. 选择 SSH key 或 sshpass 认证方式。
5. 点击 **verify**，确认显示“SSH 验证成功”。
6. 保存并切换到这个 Workspace。
7. 点击 Workspace 旁的连接按钮。

![创建远端 Workspace，并填写 SSH 主机、用户、端口和密钥](/images/remote-workspace-dialog.png)

推荐优先使用 SSH key。它更适合长期连接机器人或开发服务器，也便于在密码变化后继续使用。

## 连接会发生什么

Harbor 只在点击连接按钮后执行以下阶段：

1. 测试 SSH 是否可用。
2. 检查 `http://<host>:29385` 上是否已有 Core。
3. 检查运行中 Core 的版本与访问租约；其他 Harbor 正在使用时立即停止，不替换它。
4. 运行中 Core 不可复用时，检查远端是否已经安装匹配 release。
5. 已有匹配 release 就直接唤醒；只有缺失或 SHA-256 不匹配时才复制到 `~/.harbor`。

![Harbor 远端连接流程](/images/remote-flow.svg)

首次切换到尚未连接的远端 Workspace 时，不会自动连接、复制或唤醒 Core。已经连接过的 Workspace 会在后台保持访问租约；切换回来时，Harbor 会恢复该连接，并让远端 Core 使用对应 Workspace。若 PID 文件指向仍存活但 API 不可达的 Core，Harbor 会拒绝自动替换，避免两个版本互相抢占。

连接按钮也是手动断开按钮：连接后再次点击，会释放当前远端 Workspace 的访问租约并关闭它的终端与界面隧道。关闭 Harbor GUI 时会释放本次 GUI 建立的全部远端连接；这两种操作都不会停止远端 Core 或已经运行的 Task。

## 添加远端项目

连接成功后，打开搜索路径并添加**远端机器上的路径**：

```text
~/robot_ws
```

这里的 `~` 属于远端 SSH 用户，不是当前电脑。路径补全列出的也是远端目录。

Harbor 会在这些路径下发现 `harbor_taskcfg`。任务出现后，启动、停止、config、Group、日志和界面入口的操作方式都与本地一致。

## 在 Harbor 中打开远端终端

远端 Workspace 的工具栏会显示终端按钮。点击后，GUI 请求远端 `harbor_core` 确保
机器级 `ttyd` 可用，再建立仅监听本机 `127.0.0.1` 的 SSH Tunnel，并在 Harbor
内置窗口中打开终端。

你不需要在远端安装或配置 `ttyd`。Harbor 自带匹配的 release 二进制；第一次打开
终端时会检查 SHA-256，并在缺失或版本不匹配时自动部署到远端
`~/.harbor/tools/ttyd/<sha256>/`。它是经过校验的静态二进制，不依赖远端的软件包版本。

终端默认进入 Workspace 的第一条搜索路径，没有搜索路径时进入远端用户 HOME。每台
机器同时只允许一个终端客户端。关闭终端窗口时，Harbor 会保留 ttyd 与 SSH Tunnel，
再次打开会直接复用；切换 Workspace、主动断开远端、修改连接配置或退出 GUI 时会关闭
本机 Tunnel，但不会终止 Core 托管的 ttyd。下一次从其他 Workspace 打开终端时，Core
会按新的工作目录重启 ttyd；Core 退出时会统一关闭 ttyd。检查、部署、启动和失败原因
都会写入 Harbor Log；底层 SSH/ttyd 输出保存在远端
`~/.harbor/log/workspace-terminal.log`。

ttyd 固定使用 Harbor 默认端口 `29386`，并且只绑定远端 `127.0.0.1`，不会额外向
局域网开放 Shell 端口。如果该端口已被其他程序占用，Harbor 会直接提示冲突，不会
静默改用其他端口。浏览器访问的仍是 Harbor 动态分配的本机回环 Tunnel 端口，认证由
Workspace 已保存的 SSH 配置完成。

## 判断连接状态

界面右下角会显示当前 Core 的连接目标和状态。正常状态应包含：

- 已连接的主机名或 IP。
- Core 可达并且版本匹配。
- 当前 Workspace 对应的远端任务与日志。

如果显示“版本不符”“正在使用”或“不可达”，可以查看 Harbor Log 中逐阶段记录的连接过程：

- **检查连接**：只读取远端 Core 状态，不修改远端内容。
- **连接**：测试 SSH、检查 Core 和版本、取得租约，必要时才部署匹配 release。

## 远端文件

本机文件管理器不能直接打开远端 Linux 路径，因此 `xdg-open` 可能返回 exit status `4`。这不影响 Harbor 启动远端任务。

需要编辑或浏览远端文件时，可以使用：

- 编辑器的 Remote SSH 功能。
- 支持 SFTP 的文件管理器。
- Harbor 内置远端终端或普通 SSH 终端中的命令行工具。

Harbor 的搜索路径用于发现和运行项目，不会把远端目录挂载到本机。

## 远端程序是否需要界面

远端不代表一定需要 VNC。先判断程序本身属于哪一种：

- 服务、脚本、ROS 2 节点等没有 UI：不配置 interface，直接运行。
- 程序自己提供 HTTP 页面：配置 `webview_interface`。
- Qt、RViz、GTK 等只有原生窗口，并且需要远端操作：配置 `vnc_interface`。

如果远端没有桌面会话，程序可能输出：

```text
qt.qpa.xcb: could not connect to display
```

远端 Core 启动后会同时维护两个独立入口：

- **真实桌面**：镜像远端物理显示器 `DISPLAY=:0`，顶部使用显示器图标。
- **虚拟桌面**：Harbor 创建的 `DISPLAY=:82`，顶部使用立方体图标。

声明了 `vnc_interface` 的 Task 会把原始命令运行到虚拟桌面。程序无需读取 VNC
配置，多个 VNC Task 会作为不同窗口出现在同一个虚拟桌面中。真实桌面入口仅用于
查看和操作远端机器已经登录的物理桌面，不改变 Task 的启动位置。
local workspace 会忽略 VNC 包装，仍然直接打开原生窗口。

### 默认端口

- `23682`：虚拟桌面的 noVNC HTTP/WebSocket 端口。
- `23683`：真实桌面的 noVNC HTTP/WebSocket 端口。

它们都不是 TigerVNC 的 RFB 端口。每台远端机器只运行一组机器级桌面服务，Task
YAML 中无需也不能重复配置端口。

Harbor GUI 打开顶部显示器图标时，会通过 Workspace 的 SSH 连接，把远端
`127.0.0.1:23682` 或 `127.0.0.1:23683` 转发到本机随机空闲端口。通常不需要在
远端防火墙中开放这两个端口，也不需要手动访问远端 URL。端口被其他程序占用时，
对应桌面无法启动，错误会写入 Harbor 日志。

TigerVNC 本身不监听 TCP 端口，只通过 Harbor 运行时目录中的 Unix Socket 与
websockify 通信。本地 Workspace 不会使用 `23682`。

如果程序可以提供 Web 页面，优先使用 `webview_interface`；它通常比传输整个桌面
更轻量。完全不需要界面的程序应使用 offscreen 或 headless 模式，不要配置 VNC。

远端 VNC 依赖：

```bash
sudo apt-get install -y tigervnc-standalone-server novnc websockify openbox util-linux
```

查看真实 `DISPLAY=:0` 还需要 TigerVNC 抓屏服务：

```bash
sudo apt install tigervnc-scraping-server
```

缺少该依赖时，虚拟桌面仍可正常使用；真实桌面图标会显示安装提示，Harbor 日志也会
记录相同命令。真实桌面目前要求远端存在活动的 X11 `:0` 会话，Wayland 会话不支持
此抓取方式。

Harbor 优先使用 Ubuntu Desktop Session；不可用时依次回退到 GNOME、GNOME
Flashback 和 Openbox。若希望获得接近正常 Ubuntu Desktop 的显示效果，安装：

```bash
sudo apt-get install -y ubuntu-session gnome-shell-extension-ubuntu-dock \
  yaru-theme-gnome-shell gnome-session gnome-session-flashback dbus-x11
```

如需明确选择其他 GNOME Session，可设置 `HARBOR_VNC_DESKTOP_SESSION`；设置为
`openbox` 可强制使用轻量桌面。

## 安全边界

远端 `harbor_core` 当前没有访问认证。任何能连接远端 `29385` 端口的设备，都可能查看状态并起停任务。

- 只在可信局域网、VPN 或受控防火墙内使用。
- 不要把 `29385` 直接暴露到公网。
- WebView 或 noVNC 页面监听远端网卡时，同样需要限制网络访问或自行实现鉴权。
- Harbor 远端终端通过 SSH Tunnel 连接，ttyd 本身只监听远端回环地址；不要手动把它改成 `0.0.0.0`。

## 常见问题

### SSH 验证失败

先在终端中使用相同 host、user、port 和 identity file 测试 SSH。处理主机指纹、密钥权限和登录错误后，再回到 Harbor 验证。

### 连接时复制 Core

只有匹配 release 缺失或校验不一致时才复制。重复发生时，检查远端 `~/.harbor` 是否可写、磁盘空间是否充足，以及 Harbor Log 中的 SHA-256 和 runtime fingerprint。

### SSH 成功，但 Core 不可达

确认当前电脑可以访问远端 `29385`，并检查防火墙、容器端口和网络路由。SSH 可达不代表 HTTP 端口一定可达。

### 找不到远端 Task

确认搜索路径填写的是远端路径，并且项目中存在 `harbor_taskcfg`。不要填写当前电脑上的项目路径。

### 远端终端无法打开

确认远端 `~/.harbor` 可写、磁盘空间充足，并且远端具有 `ss` 命令。Harbor 会自动避开
已占用的终端端口，并把托管 ttyd 的部署与启动日志写入本机
`~/.harbor/log/workspace-terminal.log`。

### Task 启动但无法打开窗口

如果程序需要原生窗口，确认 Task 配置了 `vnc_interface`，并检查远端 VNC 依赖。
如果程序只需要控制页面，使用 `webview_interface` 通常更轻量；如果没有 UI，则不应配置任何 interface。
