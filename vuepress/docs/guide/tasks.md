---
title: tasks
createTime: 2026/09/19 23:55:58
permalink: /guide/1clfnrrj/
---
# 任务

Task YAML 位于 `harbor_taskcfg/tasks/`。完整字段见 [Task YAML 参考](../reference/task-yaml.md)。

![Task 生命周期](/images/task-lifecycle.svg)

## 命令形式

简单程序优先使用 `argv`，无需 shell 解析：

```yaml
command:
  argv: [python3, -u, main.py]
```

需要初始化环境、管道或多条命令时使用脚本：

```yaml
command:
  shell: bash
  script: |
    source /opt/ros/humble/setup.bash
    source install/setup.bash
    ros2 launch robot bringup.launch.py
```

## 运行配置

`configs` 用于同一任务的少量稳定变体，例如开发与生产模式。环境变量覆盖顺序为：

```text
task.env → config.env → group task env
```

同一 Task 同时只能运行一个 config。更改选择后，下一次启动或重启生效。

## 工作目录

推荐项目任务使用：

```yaml
workdir: $(harbor_taskcfg_dir)/..
```

相对路径相对于项目根，`"~"` 表示当前用户 HOME。YAML 中的 `~` 必须加引号。

## 停止与重启

停止操作会终止 Core 托管的进程树。重启使用当前选中的 config 和环境变量重新创建进程，不复用旧进程状态。
