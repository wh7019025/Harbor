# VNC Interface 示例

这个示例演示 Harbor `vnc_interface` 为远端桌面程序提供的共享桌面能力。目标程序只需要正常
启动，不需要读取端口、配置 VNC 或知道 noVNC 的存在。

```yaml
vnc_interface:
  - panel_name: desktop
command:
  argv:
    - obsidian
```

运行规则：

1. local workspace 直接启动原始程序，使用本机图形会话，不启动 VNC。
2. remote workspace 按需启动当前机器唯一的共享 X11 虚拟显示屏。
3. 远端 Core 把同一个 `DISPLAY` 注入所有 VNC Task，并在 Harbor 固定端口 `23682` 发布 noVNC 页面。
4. 停止 Task 只关闭该程序；共享桌面继续运行，并可被之后启动的 VNC Task 复用。

## 依赖

运行 Task 的机器需要安装：

```bash
sudo apt-get install -y tigervnc-standalone-server novnc websockify openbox
```

目标程序本身仍需正常安装。本示例使用 Obsidian，也可以替换为任意 X11/Qt/GTK
桌面程序。
