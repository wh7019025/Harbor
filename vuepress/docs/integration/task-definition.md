---
title: 定义 Task
permalink: /integration/task-definition/
createTime: 2026/09/20 00:21:48
---
# 定义 Task

## 目录结构

在程序仓库中创建：

```text
your-project/
├── harbor_taskcfg/
│   └── tasks/
│       └── app.yaml
├── src/
└── ...
```

## 最小配置

```yaml
version: "0.2.0-preview"
id: my-app
name: My App
description: 启动项目主程序
workdir: $(harbor_taskcfg_dir)/..
env: {}
sudo: false
command:
  argv: [python3, -u, main.py]
```

首次扫描时，Core 会自动生成 `uuid` 并写回 YAML。不要从其他任务复制 UUID。

## 选择命令形式

程序可以直接执行时使用 `argv`：

```yaml
command:
  argv: [./build/my_app, --config, config/default.yaml]
```

需要加载环境或执行多条命令时使用脚本：

```yaml
command:
  shell: bash
  script: |
    source .venv/bin/activate
    exec python3 -u main.py
```

脚本最后使用 `exec` 可让主程序直接接收 Harbor 的停止信号。

## 让 Harbor 发现程序

把项目目录或其上级目录加入 Workspace 的搜索路径，然后刷新。Harbor 最多向下扫描 5 层寻找 `harbor_taskcfg`。

完整字段见 [Task YAML](../reference/task-yaml.md)。
