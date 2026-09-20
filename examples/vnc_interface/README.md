# VNC Interface 示例

这个示例演示 Harbor `vnc_interface` 为远端桌面程序提供的临时可视化能力。目标程序只需要正常
启动，不需要读取端口、配置 VNC 或知道 noVNC 的存在。

```yaml
vnc_interface:
  - panel_name: desktop
    interface_port: 23682
command:
  argv:
    - obsidian
```

运行规则：

1. local workspace 直接启动原始程序，使用本机图形会话，不启动 VNC。
2. remote workspace 创建隔离的 X11 虚拟显示屏。
3. 远端 Core 把 `DISPLAY` 注入程序，并启动 TigerVNC、noVNC 和 WebSocket 服务。
4. 停止远端 Task 后清理全部显示进程和临时文件。

## 依赖

运行 Task 的机器需要安装：

```bash
sudo apt-get install -y tigervnc-standalone-server novnc websockify openbox
```

目标程序本身仍需正常安装。本示例使用 Obsidian，也可以替换为任意 X11/Qt/GTK
桌面程序。
