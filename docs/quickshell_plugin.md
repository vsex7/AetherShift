# Omarchy Quickshell Plugin Integration Guide

本文档介绍 **AetherShift** 与 **Omarchy 4.x (Quattro)** 原生桌面环境组件（基于 Quickshell 的状态栏与面板系统）的集成方式与扩展机制。

---

## 1. Omarchy Quickshell 插件体系背景

Omarchy 4.x 使用 Quickshell 构建了一套模块化、响应式且具有高度统一设计语言的现代化桌面 Shell（位于 `/usr/share/omarchy/shell/`）。

第三方与用户级插件通常部署在：
```text
~/.config/omarchy/plugins/<plugin-id>/
```
插件通过 `manifest.json` 声明自身属性、支持的类型（如 `bar-widget`, `panel`, `overlay`, `service`）和入口 QML 组件。

---

## 2. 插件安装与链接

AetherShift 提供了专用的 Quickshell 状态栏微件（Bar Widget），源码位于：
```text
extras/omarchy-plugin/omarchy.aethershift/
```

### 自动化安装
当运行 `./scripts/install.sh` 时，脚本会自动检测系统是否存在 `/usr/share/omarchy/shell/`，若存在则自动在用户目录建立软链接：
```bash
ln -sfn "$PWD/extras/omarchy-plugin/omarchy.aethershift" "$HOME/.config/omarchy/plugins/omarchy.aethershift"
```

### 手动安装与启用
若需要手动配置，可执行以下命令：
```bash
mkdir -p ~/.config/omarchy/plugins
ln -sfn "$PWD/extras/omarchy-plugin/omarchy.aethershift" ~/.config/omarchy/plugins/omarchy.aethershift
```

---

## 3. 插件元数据规范 (`manifest.json`)

在 `extras/omarchy-plugin/omarchy.aethershift/manifest.json` 中配置如下标准规范：

```json
{
  "schemaVersion": 1,
  "id": "omarchy.aethershift",
  "name": "AetherShift",
  "version": "0.1.0",
  "author": "AetherShift Contributors",
  "description": "Dynamic desktop paradigm switcher for Omarchy & Hyprland",
  "kinds": [
    "bar-widget"
  ],
  "entryPoints": {
    "barWidget": "BarWidget.qml"
  },
  "barWidget": {
    "displayName": "AetherShift",
    "description": "Dynamic desktop paradigm switcher",
    "category": "Compositor",
    "allowMultiple": false
  }
}
```

---

## 4. 插件与 AetherShift Daemon 的通讯方式

插件通过与后台守护进程交互实现状态同步与快速切换。推荐交互链路如下：

### 方案 A：通过 CLI 快速驱动（最简、低耦合）
Quickshell 组件可使用 `Quickshell.execDetached` 或进程执行接口调用 `aethershift` CLI：
- **获取当前状态**：
  ```bash
  aethershift status --json
  ```
- **点击切换下一个 Profile**：
  ```bash
  aethershift cycle
  ```
- **右键选择指定 Profile**：
  ```bash
  aethershift switch windows
  aethershift switch macos
  ```
- **中键一键恢复基线**：
  ```bash
  aethershift restore
  ```

### 方案 B：Unix Domain Socket 异步订阅
AetherShift Daemon 监听套接字：
```text
/run/user/$UID/aethershift.sock
```
客户端或 QML 扩展可通过 QLocalSocket / Unix Socket 建立长连接，向 Daemon 发送 JSON-RPC 风格指令，并在每次发生 Profile 切换或窗口吸附事件时即时接收状态广播。

---

## 5. 配置 Omarchy 状态栏加载组件

若要在 Omarchy 顶部/底部状态栏（Bar）中常驻展示 AetherShift 指示器：
1. 打开 Omarchy Shell 配置文件 `~/.config/omarchy/shell.json`；
2. 在 `bar.start`、`bar.center` 或 `bar.end` 数组中添加 `"omarchy.aethershift"`，例如：
   ```json
   {
     "bar": {
       "end": [
         "omarchy.aethershift",
         "omarchy.audio",
         "omarchy.network",
         "omarchy.clock"
       ]
     }
   }
   ```
3. 保存文件后，Omarchy Quickshell 具有文件监视能力，将自动热重载并在状态栏中渲染 AetherShift 状态图标与交互微件。
