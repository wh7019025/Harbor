---
title: groups
createTime: 2026/09/19 23:55:58
permalink: /guide/f1t0kpna/
---
# 任务组

Group YAML 位于 `harbor_taskcfg/groups/`，用于可重复地启动一组 Task。

```yaml
version: "0.2.0-preview"
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

## 引用解析

1. 优先查找与 Group 位于同一 `harbor_taskcfg` 的同 id Task。
2. 同目录没有时，在已发现任务中搜索。
3. 只有一个候选时使用该任务。
4. 存在多个候选时拒绝执行，并显示候选位置。

Harbor 会在第一个任务启动前验证完整 Group，避免执行到一半才发现引用错误。

## 停止行为

停止 Group 会停止该 Group 管理的任务。若任务已经独立运行或被其他流程接管，应以界面显示的实时状态为准。
