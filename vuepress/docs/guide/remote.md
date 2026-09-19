---
title: 远端运行
permalink: /guide/remote/
createTime: 2026/09/20 00:44:43
---
# 远端运行

远端 Workspace 让你在当前电脑的 Harbor 中，直接管理机器人或服务器上的 Task。

项目文件和程序仍然留在远端机器上。Harbor 通过 SSH 准备远端 `harbor_core`，连接完成后，任务发现、启动、停止和日志读取都由远端 Core 执行。

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
- 使用 Web Panel 时，对应 Panel 端口也需要可达。

如果这些网络路径被防火墙、容器或路由隔离，Harbor 无法替代底层网络配置。

## 创建远端 Workspace

1. 点击 Workspace 旁的 `+`。
2. 输入 Workspace 名称，模式选择 **remote**。
3. 填写 SSH host、user 和 port。
4. 选择 SSH key 或 sshpass 认证方式。
5. 点击 **verify**，确认显示“SSH 验证成功”。
6. 保存并切换到这个 Workspace。

![创建远端 Workspace，并填写 SSH 主机、用户、端口和密钥](/images/remote-workspace-dialog.png)

推荐优先使用 SSH key。它更适合长期连接机器人或开发服务器，也便于在密码变化后继续使用。

## 第一次连接会发生什么

Harbor 会自动完成以下工作：

1. 通过 SSH 检查远端机器和已有 Core。
2. 比较 Harbor GUI 所需的版本、API revision 和 SHA-256。
3. 远端缺少匹配 Core 时，把配套的 release `harbor_core` 和运行依赖复制到远端 `~/.harbor`。
4. 启动远端 Core，并通过 `http://<host>:29385` 连接。

![Harbor 远端连接流程](/images/remote-flow.svg)

正常切换到已经准备好的远端 Workspace 时，不会重复复制 Core。只有远端 Core 缺失、校验不匹配，或用户主动执行“部署并重启”时才会上传。

## 添加远端项目

连接成功后，打开搜索路径并添加**远端机器上的路径**：

```text
~/robot_ws
```

这里的 `~` 属于远端 SSH 用户，不是当前电脑。路径补全列出的也是远端目录。

Harbor 会在这些路径下发现 `harbor_taskcfg`。任务出现后，启动、停止、config、Group、日志和 Web Panel 的操作方式都与本地一致。

## 判断连接状态

界面右下角会显示当前 Core 的连接目标和状态。正常状态应包含：

- 已连接的主机名或 IP。
- Core 可达并且版本匹配。
- 当前 Workspace 对应的远端任务与日志。

如果显示“版本不符”“不可达”或持续“正在复制”，先打开设置中的 Harbor Core 区域：

- **检查连接**：只读取远端 Core 状态，不修改远端内容。
- **部署并重启**：在需要时上传匹配版本，并替换远端 Core。

## 远端文件

本机文件管理器不能直接打开远端 Linux 路径，因此 `xdg-open` 可能返回 exit status `4`。这不影响 Harbor 启动远端任务。

需要编辑或浏览远端文件时，可以使用：

- 编辑器的 Remote SSH 功能。
- 支持 SFTP 的文件管理器。
- SSH 终端中的命令行工具。

Harbor 的搜索路径用于发现和运行项目，不会把远端目录挂载到本机。

## 远端图形程序

Qt、RViz 和其他桌面程序需要远端机器上存在真实图形会话。仅能 SSH 登录并不代表可以打开窗口。

如果远端没有桌面会话，程序可能输出：

```text
qt.qpa.xcb: could not connect to display
```

可根据程序类型选择：

- 无需显示界面：使用 offscreen 或 headless 模式。
- 必须显示窗口：在远端配置物理桌面、VNC 或 RDP 会话。
- 只需要操作界面：优先把功能做成 [Web Panel](./panels.md)，直接在 Harbor 中打开。

Harbor 会尝试继承远端用户已有的 `DISPLAY`、`XAUTHORITY`、Wayland 和 DBus 环境，但不会创建虚拟显示设备。

## 安全边界

远端 `harbor_core` 当前没有访问认证。任何能连接远端 `29385` 端口的设备，都可能查看状态并起停任务。

- 只在可信局域网、VPN 或受控防火墙内使用。
- 不要把 `29385` 直接暴露到公网。
- Web Panel 若监听 `0.0.0.0`，同样需要限制网络访问或自行实现鉴权。

## 常见问题

### SSH 验证失败

先在终端中使用相同 host、user、port 和 identity file 测试 SSH。处理主机指纹、密钥权限和登录错误后，再回到 Harbor 验证。

### 一直显示正在复制 Core

正常情况下只有首次连接或版本变化时复制。重复发生时，检查远端 `~/.harbor` 是否可写、磁盘空间是否充足，以及 Harbor Log 中的上传和 SHA-256 校验错误。

### SSH 成功，但 Core 不可达

确认当前电脑可以访问远端 `29385`，并检查防火墙、容器端口和网络路由。SSH 可达不代表 HTTP 端口一定可达。

### 找不到远端 Task

确认搜索路径填写的是远端路径，并且项目中存在 `harbor_taskcfg`。不要填写当前电脑上的项目路径。

### Task 启动但无法打开窗口

检查远端图形会话和 `DISPLAY`。如果程序只需要一个控制页面，使用 Web Panel 通常比维护远端桌面更简单。
