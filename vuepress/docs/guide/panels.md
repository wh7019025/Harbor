---
title: panels
createTime: 2026/09/19 23:55:59
permalink: /guide/uc5scp38/
---
# Web 面板

Task 可通过 `panel_interface` 声明一个或多个 HTTP 面板。Harbor 负责展示入口和构造地址，面板服务仍由 Task 自己启动。

```yaml
panel_interface:
  - panel_name: robot_panel
    interface_port: 23681
    localhost_only: false
```

## 注入的环境变量

单面板任务会获得：

- `HARBOR_PANEL_NAME`
- `HARBOR_PANEL_INTERFACE_PORT`
- `HARBOR_PANEL_LOCALHOST_ONLY`

多面板任务还会获得带标准化面板名的变量。例如 `robot-panel` 对应 `HARBOR_PANEL_ROBOT_PANEL_INTERFACE_PORT`。

面板程序应读取这些变量，不要在源码中重复保存端口。

## 地址规则

- 本地 Workspace：`http://127.0.0.1:<port>/`
- 远端 Workspace：`http://<ssh-host>:<port>/`

当 `localhost_only: true` 时，服务只应监听环回地址，因此远端浏览器不能直接访问。

## 示例

仓库中的 `examples/webpage_view` 展示了动态视频、WebSocket 通信、ROS 2 接入和无 ROS 环境下的 mock 数据。
