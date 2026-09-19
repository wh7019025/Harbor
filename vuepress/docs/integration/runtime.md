---
title: 运行时约定
permalink: /integration/runtime/
createTime: 2026/09/20 00:21:48
---
# 运行时约定

## 标准输出与错误输出

程序应把运行状态写到 stdout，把错误和诊断信息写到 stderr。Harbor 会合并采集并写入 Workspace 日志。

- 长时间运行程序应及时刷新输出。
- Python 推荐使用 `python3 -u` 或设置 `PYTHONUNBUFFERED=1`。
- 不要只把日志写入程序自己的隐藏文件，否则 Harbor 中无法实时观察。

## 正确处理停止信号

Harbor 停止 Task 时会终止托管的进程树。程序应处理常规终止信号，释放相机、串口、共享内存和网络端口。

Shell 脚本应尽量用 `exec` 启动最终进程：

```bash
exec ./my_app
```

## 环境变量

顶层 `env` 保存通用环境，`configs` 保存稳定运行变体：

```yaml
env:
  ROBOT_ID: robot-01
configs:
  - id: simulation
    name: 仿真
    env:
      MODE: sim
  - id: hardware
    name: 实机
    env:
      MODE: hardware
default_config: simulation
```

覆盖顺序为：`task.env → config.env → group task env`。

## 图形程序

Core 会继承当前用户图形会话中的 `DISPLAY`、`XAUTHORITY`、Wayland 和 DBus 环境。远端运行 GUI 程序时，远端用户仍必须存在真实桌面、VNC 或 RDP 会话。

## 退出码

程序应使用 `0` 表示正常结束，非零值表示错误。即使任务很快退出，也应在退出前打印可定位原因的信息。
