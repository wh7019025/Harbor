---
title: 工作空间
permalink: /guide/workspaces/
createTime: 2026/09/20 00:44:43
---
# 工作空间

Workspace 决定两件事：**连接哪台机器，以及在这台机器上查看哪些项目。**

你可以为日常开发、机器人本体或不同项目分别创建 Workspace。切换时，Harbor 会显示对应机器上的任务和日志，不需要重新配置项目。

## 什么时候需要 Workspace

### 区分不同项目

为每个 Workspace 添加不同的项目路径，只显示当前工作需要的任务。

例如：

- `simulation`：仿真、算法和测试工具。
- `robot`：驱动、传感器和控制程序。
- `demo`：演示需要的一组稳定任务。

### 管理不同机器

本地 Workspace 连接当前电脑，远端 Workspace 连接机器人或开发服务器。

切换到远端后，任务扫描、启动和日志读取都发生在远端机器上。操作方式与本地一致。

## 添加项目

1. 进入当前 Workspace 的设置。
2. 在搜索路径中添加项目目录，或多个项目共同的上级目录。
3. Harbor 自动查找路径下的 `harbor_taskcfg`，并在任务列表中显示发现的 Task 与 Group。

推荐直接添加项目目录：

```text
~/workspace/robot_driver
```

如果多个项目放在同一目录，也可以添加它们共同的上级目录：

```text
~/workspace
├── robot_driver/harbor_taskcfg/
├── perception/harbor_taskcfg/
└── teleop/harbor_taskcfg/
```

::: tip
搜索范围越明确，扫描越快。不要直接添加整个 HOME、根目录或大型挂载盘。
:::

## 切换 Workspace 会发生什么

切换 Workspace 只改变 Harbor 当前显示和管理的范围：

- 切换连接的本地或远端机器。
- 使用该 Workspace 自己的搜索路径。
- 本地 Workspace 显示自己的历史日志；远端 Workspace 显示对应远端机器上的任务日志。

**正在运行的任务不会因为切换 Workspace 而停止。**

已经连接的远端 Workspace 也不会因为切换而断开。Harbor 会在后台维持它的访问租约；再次切换回来时，会直接恢复任务、日志和界面状态，不需要重新连接。

只有以下操作会释放远端连接：

- 再次点击 Workspace 旁的连接按钮，主动断开当前远端 Workspace。
- 关闭 Harbor GUI，统一释放本次 GUI 建立的全部远端连接。
- 修改或删除对应 Workspace。

释放连接不会停止远端 `harbor_core`，也不会停止已经运行的 Task。

如果另一个 Workspace 也发现了同一个 Task，Harbor 会根据 UUID 识别它，并显示同一份实时运行状态，而不是启动第二个实例。

## 本地与远端

### 本地 Workspace

适合开发、调试和运行本机程序。Harbor 默认只允许本机访问 Core。

### 远端 Workspace

适合操作机器人或服务器。首次使用远端 Workspace 时，需要点击 Workspace 旁的连接按钮；Harbor 会先检查 SSH、Core 和版本，必要时才准备远端 Core。连接会一直保留到你主动断开或关闭 Harbor GUI。

远端 Workspace 中的 `~`、项目路径和日志都属于远端用户，而不是当前电脑。配置方法见 [远端运行](./remote.md)。

Workspace 定义只在运行 Harbor GUI 的电脑上维护。连接远端时，GUI 会把当前 Workspace
的名称和搜索路径发送给远端 Core 作为运行时视图；远端不会成为第二份 Workspace 配置
来源。远端持久化的是 Task 日志、Core 日志和进程接管信息。

## Workspace 不会做什么

- 不会复制或移动项目文件。
- 不会修改项目中的 Task 和 Group 定义。
- 不会为同一个任务创建互相隔离的运行实例。
- 不会在切换时停止已经运行的任务。

可以把 Workspace 理解为 Harbor 中保存的一组**机器连接与项目视图**。项目配置仍然保存在代码仓库的 `harbor_taskcfg` 中，任务运行状态则由所在机器的 `harbor_core` 统一管理。
