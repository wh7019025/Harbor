---
title: 任务
permalink: /guide/tasks/
createTime: 2026/09/20 00:44:43
---
# 任务

Task 是 Harbor 中最基本的运行单元，表示一个可以反复启动的程序。

驱动、ROS 2 Launch、Python 脚本、Web 服务或一条调试命令，都可以成为 Task。Harbor 会记住它的运行目录、环境和启动方式，让你不必每次重新打开终端、切换目录和输入命令。

::: important 核心规则
**一个 Task 在同一时刻只能有一个运行实例。**

无论从哪个 Workspace、GUI 或 API 发起启动，Harbor 都会通过 Task 的 UUID 识别它。任务已经运行时，不会再创建第二个实例；需要应用新命令、环境或 config 时，应使用 Restart。
:::

## 什么时候创建 Task

当一段程序需要被重复运行时，就适合创建 Task。例如：

- 启动相机或底盘驱动。
- 运行感知、规划或控制节点。
- 启动仿真、数据记录或 Web 服务。
- 执行需要固定环境和工作目录的脚本。

一个 Task 对应一个独立程序。需要按顺序运行多个 Task 时，再使用 [Group](./groups.md) 进行编排。

## Task 会记住什么

- **如何启动**：可执行程序和参数，或一段 Shell 脚本。
- **从哪里启动**：程序使用的工作目录。
- **需要什么环境**：环境变量和初始化脚本。
- **有哪些运行方式**：例如开发、生产或不同设备配置。
- **如何查看界面**：可选的 Web Panel。

这些内容保存在项目的 `harbor_taskcfg/tasks/` 中，可以与代码一起提交和维护。

## 创建 Task

1. 确认项目已经加入当前 Workspace 的搜索路径。
2. 在 Harbor 的 **Tasks** 区域点击 `+`。
3. 选择保存位置，填写名称、工作目录和启动命令。
4. 保存后，Task 会出现在任务列表中。

Harbor 会为新 Task 生成 UUID。不要从其他 Task 复制 UUID，它是 Harbor 判断“这是不是同一个 Task”的依据。

如果更习惯直接描述目标，也可以让 AI 完成创建和验证：

```txt
使用 $harbor，为当前项目创建一个 Task。
```

## 启动与观察

点击运行按钮后，Harbor 会启动 Task 并显示实时状态与日志。

- **Start**：使用当前选择的 config 启动任务。
- **Stop**：停止 Task 以及它创建的进程树。
- **Restart**：停止旧进程，再用当前配置重新启动。

如果 Task 已经运行，再次 Start 不会产生第二个实例。切换 Workspace 也不会停止它；只要另一个 Workspace 发现了相同 UUID，就会显示同一个运行状态。

![Task 在 Stopped 与单实例 Running 状态之间切换](/images/task-lifecycle.svg)

## 使用运行配置

同一个程序需要几种稳定运行方式时，可以为 Task 添加 config。例如：

- 相机使用 `front` 或 `rear` 设备。
- 控制器使用 `simulation` 或 `robot` 参数。
- 服务使用 `development` 或 `production` 环境。

启动前在 Task 上选择 config。一个 Task 同时只能运行一种 config；切换选择后，需要重启 Task 才会生效。

![在 Task 中选择标准运行、快速运行或长时间运行配置](/images/task-config-selector.png)

如果两个运行方式已经变成不同程序，应该拆成两个 Task，而不是继续增加 config。

## 手动编辑 YAML

简单程序使用参数列表，命令会被直接执行：


```yaml
command:
  argv: [python3, -u, main.py]
```

需要初始化环境、执行多条命令或使用 Shell 语法时，改用脚本：

```yaml
command:
  shell: bash
  script: |
    source /opt/ros/humble/setup.bash
    source install/setup.bash
    ros2 launch robot bringup.launch.py
```

项目内程序通常把工作目录设置为项目根目录：

```yaml
workdir: $(harbor_taskcfg_dir)/..
```

完整字段、config 和环境覆盖规则见 [Task YAML 参考](./reference/task-yaml.md)。
