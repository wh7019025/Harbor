---
title: Harbor Skill
permalink: /integration/agent-skill/
createTime: 2026/09/20 01:30:00
---
# Harbor Skill

## 创建 Task

```txt
使用 $harbor，为当前项目创建一个 Task，用 uv 运行 main.py。
```

```txt
使用 $harbor，把 scripts/start_camera.sh 配置成 Harbor Task，工作目录使用项目根目录，并保留当前环境变量。
```

## 创建 Group

```txt
使用 $harbor，把 camera、perception 和 controller 编排成一个 Group，按顺序启动。
```

```txt
使用 $harbor，创建一个 teleop Group。先启动底盘驱动，再启动手柄节点，并为手柄任务选择 dpvr config。
```

## 接入 ROS 2

```txt
使用 $harbor，为这个 ROS 2 package 创建一个 Task。启动前 source /opt/ros/humble/setup.bash 和 install/setup.bash，然后运行 ros2 launch。
```

## 接入 Web Panel

```txt
使用 $harbor，为当前程序增加一个 Web Panel Task。后端通过 WebSocket 推送视频帧，前端显示实时画面。
```

## 操作任务

```txt
使用 $harbor，启动当前项目的 perception Task，并确认它进入 running 状态。
```

```txt
使用 $harbor，停止 teleop Group，然后修改 controller Task 的启动参数并重新启动。
```

## 查看日志

```txt
使用 $harbor，检查 camera Task 最后 128 行日志，找出启动失败的原因。
```

```txt
使用 $harbor，查看 Harbor 日志，判断远端 harbor_core 为什么连接失败。
```

## 维护 Workspace

```txt
使用 $harbor，把当前项目加入当前 Workspace 的 search_paths，保留其他 Workspace 设置不变。
```
