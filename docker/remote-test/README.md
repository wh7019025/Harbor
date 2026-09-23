# Docker 远端测试机

该环境把本机 Docker 容器作为一台独立远端机器，用真实 SSH、远端 `harbor_core`
和共享 VNC/noVNC 验证 Ubuntu Desktop 及 `xclock` 窗口。

## 启动

```bash
npm run remote:test:up
npm run remote:test:dev
```

当前用户需要有 Docker 权限。如果出现 `permission denied`，执行一次：

```bash
sudo usermod -aG docker "$USER"
```

然后注销并重新登录桌面会话，再重新运行启动命令。

`remote:test:dev` 会把 `127.0.0.2` 加入当前 GUI 进程的代理绕过列表，避免 HTTP
代理破坏 noVNC 的 WebSocket 握手。

在 Harbor 创建 remote Workspace：

| 字段 | 值 |
| --- | --- |
| Name | `docker-remote` |
| SSH Host | `127.0.0.2` |
| User | `harbor` |
| Port | `2222` |
| Auth | `sshpass` |
| Password | `harbor` |
| Search Path | `/home/harbor/workspace` |

先验证 SSH，再连接 Workspace。首次连接会把当前 GUI 匹配的 release `harbor_core`
复制到容器。连接后运行 `Ubuntu Desktop + xclock`，通过 `23682` 打开共享
Ubuntu Desktop Session，并确认桌面中的 `xclock` 正常显示。Harbor 依次尝试 Ubuntu、
GNOME、GNOME Flashback，最后才回退到 Openbox。

GNOME Shell 首次打开时可能停留在 Activities 概览，按 `Esc` 即可回到普通桌面。

容器端口只映射到 `127.0.0.2`，不会占用本机 Core 使用的
`127.0.0.1:29385`，也不会暴露到局域网。

## 查看与停止

```bash
npm run remote:test:logs
npm run remote:test:down
```
