---
title: 接入 ROS 2
permalink: /integration/ros2/
createTime: 2026/09/20 00:21:49
---
# 接入 ROS 2

ROS 2 程序通过 Task 的 shell 环境加载 ROS 和 Workspace，再启动 node 或 launch 文件。

## 启动单个节点

```yaml
version: "0.2.0-preview"
id: camera-node
name: Camera Node
description: 启动相机 ROS 2 节点
workdir: $(harbor_taskcfg_dir)/..
env:
  ROS_DOMAIN_ID: "20"
sudo: false
command:
  shell: bash
  script: |
    source /opt/ros/humble/setup.bash
    source install/setup.bash
    exec ros2 run camera_driver camera_node
```

## 启动 launch

```yaml
command:
  shell: bash
  script: |
    source /opt/ros/humble/setup.bash
    source install/setup.bash
    exec ros2 launch robot_bringup bringup.launch.py
```

## DDS 与网络

- 同一套系统应明确设置一致的 `ROS_DOMAIN_ID`。
- 远端机器的 DDS 网卡、组播和防火墙配置仍由 ROS 2 环境负责。
- 需要切换 CycloneDDS 或 Fast DDS 配置时，使用 Task `configs` 管理环境变量。

## 多节点编排

节点可以由一个 launch 统一管理，也可以拆成多个 Task 并用 Group 排序启动。需要独立重启、独立日志和独立状态时，优先拆成多个 Task。

## ROS 2 Web Panel

Panel 后端可以同时作为 ROS 2 node，订阅 Topic 后通过 WebSocket 推送给前端。无 ROS 环境时建议提供 mock 模式，便于独立开发和文档演示。
