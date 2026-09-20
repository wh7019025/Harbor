---
title: 任务组
permalink: /guide/groups/
createTime: 2026/09/20 00:44:43
---
# 任务组

Group 把多个 Task 的启动顺序保存为一次操作。

例如，机器人启动时总要依次运行底盘驱动、相机、感知和控制程序。把它们放进 Group 后，只需启动一次，Harbor 就会按照固定顺序执行，不必再逐个操作。

::: important 核心规则
**Group 负责编排 Task，但不会改变 Task 的单实例规则。**

Group 中的每个 Task 仍然是独立任务，同一时刻最多只有一个实例。Group 本身不是额外的运行进程，而是一份可重复执行的启动与停止流程。
:::

## 什么时候创建 Group

当多个 Task 经常需要一起运行，并且启动顺序相对固定时，就适合创建 Group。例如：

- 机器人 Bringup：驱动 → 传感器 → 控制器。
- 感知流程：相机 → 推理服务 → 可视化面板。
- 仿真环境：仿真器 → Robot State Publisher → 算法节点。
- 演示流程：后端服务 → WebView → 操作程序。

如果几个 Task 通常独立使用，就不必为了归类而创建 Group。Group 表达的是**一起运行的流程**，不是文件夹。

## 创建 Group

1. 先创建并确认 Group 需要引用的 Task。
2. 在 Harbor 的 **Groups** 区域点击 `+`。
3. 按启动顺序加入 Task，并按需选择 config、等待时间和环境变量。
4. 保存后，从 Group 上直接启动整套流程。

也可以让 AI 根据目标完成编排和验证：

```txt
使用 $harbor，把 camera、perception 和 controller 组成一个 Group。
```

## Group 如何启动

启动 Group 时，Harbor 会先检查所有 Task 引用和 config 是否有效，然后按列表从上到下启动。

每一项可以单独指定：

- **config**：这次启动 Task 使用哪种运行配置。
- **wait_after_sec**：启动当前 Task 后，等待多久再启动下一项。
- **env**：只为这次 Group 启动追加或覆盖环境变量。

`wait_after_sec` 是固定等待时间，不代表 Harbor 已经判断程序就绪。需要精确判断服务是否可用时，应让 Task 自身提供可靠的启动与重试机制。

如果某个 Task 已经使用相同 config 运行，Harbor 不会创建第二个实例；如果它正在使用不同 config 运行，Group 会停止启动并提示先 Stop 或 Restart。

## 停止 Group

停止 Group 时，Harbor 会按照与启动相反的顺序停止其中的 Task。

::: warning
停止 Group 会停止它引用的所有 Task，包括在启动 Group 之前已经独立运行的 Task。把 Group 当作一套完整流程使用，不要把不希望一起停止的程序加入其中。
:::

## 启动失败时

Harbor 会在启动第一个 Task 前检查完整引用，避免因为名称错误或 config 不存在而执行到一半。

程序本身仍可能在运行时启动失败。如果流程中途失败，前面已经成功启动的 Task 会保持运行，方便查看日志和定位问题。处理完成后，可以停止 Group，再重新启动整套流程。

## 手动编辑 YAML

Group 配置保存在项目的 `harbor_taskcfg/groups/` 中：

```yaml
version: "0.2.0-preview.2"
id: robot-bringup
name: Robot Bringup
description: 依次启动驱动和控制面板
tasks:
  - task: driver
    wait_after_sec: 2
    env: {}
  - task: robot-panel
    config: development
    wait_after_sec: 0
    env:
      LOG_LEVEL: debug
```

Harbor 优先使用与 Group 位于同一项目的同名 Task。如果其他项目中存在多个同名候选且无法确定引用，Group 会拒绝启动并显示冲突位置。

完整字段说明见 [Group YAML 参考](./reference/group-yaml.md)。
