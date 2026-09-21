---
title: 快速开始
createTime: 2026/09/19 23:55:58
permalink: /guide/getting-started/
---
# 快速开始

## 环境要求

- Debian 或 Ubuntu 系 Linux 桌面环境
- x86_64 / amd64 机器
- 可访问本地或远端任务目录

## 安装 Harbor

前往 [Harbor Releases](https://github.com/wh7019025/Harbor/releases)，下载最新版本的 `Harbor_<version>_amd64.deb`。

在下载目录中安装：

```bash
sudo apt install ./Harbor_<version>_amd64.deb
```

安装完成后，从应用菜单启动 Harbor。也可以在终端中启动：

```bash
harbor
```

::: tip
GitHub Releases 提供的是可直接安装的发行二进制文件。普通用户不需要安装 Node.js、Rust，也不需要从源码编译 Harbor。
:::

## 创建第一个任务

在任意项目中创建 `harbor_taskcfg/tasks/hello.yaml`：

```yaml
version: "0.2.0-preview.3"
id: hello
name: Hello Harbor
description: 每秒输出一条问候信息
workdir: $(harbor_taskcfg_dir)/..
env: {}
sudo: false
command:
  shell: bash
  script: |
    while true; do
      echo "hello from Harbor"
      sleep 1
    done
```

首次扫描时，`harbor_core` 会生成 `uuid` 并写回文件。不要从其他任务复制 UUID。

## 在 Harbor 中发现任务

1. 打开设置，为当前 Workspace 添加项目路径或其上级目录。
2. 等待扫描完成，或执行刷新。
3. 在任务列表找到 `Hello Harbor` 并启动。
4. 在详情区查看实时日志，停止任务后查看历史日志。

::: tip
搜索路径会递归寻找 `harbor_taskcfg`，最大深度为 5。把路径设置在实际项目附近可减少扫描时间。
:::
