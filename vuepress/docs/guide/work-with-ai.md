---
title: Work with AI
permalink: /guide/work-with-ai/
createTime: 2026/09/20 01:23:29
---
# Work with AI

Harbor 不只是让 AI 帮你编写 YAML。由于 `harbor_core` 提供完整的 Web API，AI 可以读取真实状态、修改配置、执行任务并检查结果，成为 Harbor 的直接操作者。

你只需要描述目标，不必把每一步操作翻译成按钮或命令。

## 从建议到执行

普通的 AI 助手只能告诉你“应该怎样做”。Harbor Skill 可以通过 Core API 直接完成：

- 读取当前 Core、Workspace、Task、Group 和 UUID 冲突。
- 创建、读取、更新和删除 Task 或 Group 配置。
- 添加搜索路径、刷新发现结果、切换 Workspace。
- 启动、停止或重启 Task，启动或停止整个 Group。
- 持续读取运行状态和增量日志，确认操作是否真正成功。
- 根据错误修改配置，再次运行并验证结果。

因此一次 AI 请求可以形成完整闭环：

```text
理解目标 → 检查状态 → 修改配置 → 执行任务 → 读取日志 → 验证结果
```

Harbor GUI 与 AI 使用同一套 API。无论操作来自界面还是 AI，你看到的都是同一个 Core 中的真实状态。

## 直接完成任务

告诉 AI 最终目标，而不是逐步指导它如何点击界面：

```txt
使用 $harbor，把当前项目接入 Harbor。
```

```txt
使用 $harbor，启动 perception。
```

这类请求不只生成文件。AI 会先读取 `/health` 与 `/snapshot`，通过 YAML API 保存配置，再调用运行 API，并根据状态与日志判断结果。

## 编排并运行流程

AI 可以先发现已有任务，再把重复操作沉淀为 Group：

```txt
使用 $harbor，把 camera、perception 和 controller 组成一个 Group。
```

```txt
使用 $harbor，修复 teleop Group。
```

## 操作本地与远端 Harbor

Core API 在本地与远端保持一致。切换 Workspace 或远端主机后，AI 仍然可以使用相同方式读取状态和操作任务：

```txt
使用 $harbor，在远端启动 robot_bringup。
```

```txt
使用 $harbor，把当前项目加入远端 Workspace。
```

## 持续观察与排障

AI 可以基于快照和增量日志观察任务，而不是依赖一次性的命令返回值：

```txt
使用 $harbor，重启 camera，看看是否正常。
```

```txt
使用 $harbor，看看当前有哪些任务失败了。
```

```txt
使用 $harbor，解决当前的 UUID 冲突。
```

## 能力边界

- 明确说出“使用 `$harbor`”，让 AI 使用 Harbor Skill 而不是只给出通用建议。
- 尽量描述期望结果和验收条件，状态检查、API 调用与日志读取由 AI 完成。
- AI 会读取实时状态，但不会绕过 Harbor 的 UUID、版本和单实例约束。
- 修改正在运行的任务前，AI 应先停止任务，避免配置与真实状态不一致。
- Harbor Core 默认没有鉴权，只应在可信网络中交给 AI 操作。

完整接口见 [Web API](./reference/web-api.md)。
