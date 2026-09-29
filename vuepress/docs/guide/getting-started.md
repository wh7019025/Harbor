---
title: 快速开始
createTime: 2026/09/19 23:55:58
permalink: /guide/getting-started/
---
# 快速开始

## 环境要求

- Linux：可以运行本地任务，也可以连接远端 Linux 主机。
- Windows / ARM64 macOS：作为远端控制端使用，不在本机启动 `harbor_core`。
- 远端模式需要可通过 SSH 访问运行任务的 Linux 主机。

## 安装 Harbor

前往 [Harbor Releases](https://github.com/wh7019025/Harbor/releases)，下载当前平台的安装包。

Linux 使用 `Harbor_<version>_amd64.deb`，在下载目录中安装：

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

## 创建 Workspace

首次打开 Harbor 时，先创建一个 Workspace：

- 在 Linux 上选择 `local`，直接管理本机任务。
- 选择 `remote`，填写 Linux 主机的 SSH 地址与认证信息，再点击连接按钮。
- Windows 与 macOS 会禁用 `local`，直接使用 `remote`。

Workspace 创建完成后，再添加项目路径并发现 Task 与 Group。

## 创建第一个任务

在任意项目中创建 `harbor_taskcfg/tasks/hello.yaml`：

```yaml
version: "0.2.1-rc1"
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

YAML 中的 `version` 应与当前 Harbor 版本一致。可以运行 `harbor --version` 查看；通过
Harbor 新建或保存 Task 时，GUI 会自动写入当前版本。

## 在 Harbor 中发现任务

1. 为当前 Workspace 添加项目路径或其上级目录。
2. 等待扫描完成，或执行刷新。
3. 在任务列表找到 `Hello Harbor` 并启动。
4. 在详情区查看实时日志，停止任务后查看历史日志。

::: tip
搜索路径会递归寻找 `harbor_taskcfg`，最大深度为 5。把路径设置在实际项目附近可减少扫描时间。
:::
