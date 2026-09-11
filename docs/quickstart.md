# AetherShift Quickstart Guide

本指南将带你快速完成 AetherShift 的安装部署、日常命令行 (CLI) 操作、TUI 控制台使用以及个性化预设定制。

---

## 1. 安装与启动

### 一键安装
在项目根目录下执行安装脚本：
```bash
./scripts/install.sh
```
该脚本会自动完成：
1. 编译 release 二进制程序（`aethershift`, `aethershift-daemon`, `aethershift-tui`）；
2. 安装二进制到用户可执行目录 `~/.local/bin/`；
3. 复制预设配置到 `~/.config/aethershift/presets/`；
4. 配置并启用 systemd 用户服务 `aethershift.service`；
5. 若检测到 Omarchy 环境，自动配置 Quickshell 插件软链接。

> **提示**：请确保 `~/.local/bin` 已经加入系统的 `PATH` 环境变量中（通常在 `~/.bashrc` 或 `~/.zshrc` 中配置 `export PATH="$HOME/.local/bin:$PATH"`）。

### 检查运行状态
```bash
aethershift status
```
或通过 systemd 检查服务：
```bash
systemctl --user status aethershift.service
```

---

## 2. CLI 常用操作指南

`aethershift` 命令行客户端响应迅速（毫秒级），以下是高频操作：

### 2.1 状态与预设切换
- **查询当前状态**：
  ```bash
  aethershift status
  # 输出 JSON 格式
  aethershift status --json
  ```
- **列出所有可用预设**：
  ```bash
  aethershift list
  ```
- **切换到指定 Profile**：
  ```bash
  aethershift switch windows
  aethershift switch macos
  aethershift switch hybrid
  aethershift switch native
  ```
- **循环切换至下一个 Profile**：
  ```bash
  aethershift cycle
  ```
- **一键恢复基线（注销所有覆盖绑定）**：
  ```bash
  aethershift restore
  ```

### 2.2 窗口高精度吸附 (Window Snapping)
AetherShift 内置 14 种精准几何布局计算引擎，支持快捷调度当前活动窗口：
```bash
# 左右半屏
aethershift snap half-left
aethershift snap half-right

# 上下半屏
aethershift snap half-top
aethershift snap half-bottom

# 三分屏矩阵
aethershift snap two-thirds-left
aethershift snap one-third-right
aethershift snap one-third-left
aethershift snap two-thirds-right
aethershift snap three-columns-left
aethershift snap three-columns-center
aethershift snap three-columns-right

# 居中浮动与最大化
aethershift snap center
aethershift snap maximize
aethershift snap restore
```

### 2.3 多显示器窗口调度
将当前窗口在多显示器之间快速传送：
```bash
aethershift monitor l   # 移至左侧显示器
aethershift monitor r   # 移至右侧显示器
aethershift monitor u   # 移至上方显示器
aethershift monitor d   # 移至下方显示器
```

### 2.4 Profile 动态管理与定制
无需手动编辑 TOML 文件，即可直接在运行时增删和持久化快捷键：
```bash
# 复制现有配置创建新 Profile
aethershift profile create mywork --copy-from windows

# 绑定新快捷键 (如 SUPER + T 打开终端)
aethershift profile bind mywork "SUPER + T" "exec:omarchy-launch-terminal" --desc "Open terminal"

# 移除指定快捷键
aethershift profile unbind mywork "SUPER + T"

# 持久化保存至 ~/.config/aethershift/presets/mywork.toml
aethershift profile save mywork

# 删除 Profile
aethershift profile delete mywork
```

### 2.5 统计与人体工学分析
- **查看按键频次与使用统计**：
  ```bash
  aethershift stats
  # 导出统计至 JSON 文件
  aethershift stats --export stats.json
  ```
- **查看智能按键优化建议**：
  ```bash
  aethershift recommend
  ```

---

## 3. TUI 控制台使用 (aethershift-tui)

执行以下命令进入全屏交互式终端控制台：
```bash
aethershift-tui
```

### 界面布局
- **Header（顶部）**：显示 Daemon 在线状态、当前激活 Profile、注入的 Overlay 绑定数以及运行时间。
- **Profiles 面板（左侧）**：列出所有已加载的 Profile，标注当前激活项与快捷键总数。
- **Snap Matrix 面板（右上）**：14 种窗口几何吸附布局的快速选择矩阵。
- **Stats & Recommendations 面板（右下）**：实时展示切换次数、高频操作榜单以及人体工学建议。
- **Footer（底部）**：交互快捷键提示与即时操作反馈消息条。

### 快捷键清单

| 按键 | 功能描述 |
| :--- | :--- |
| `Tab` | 在左侧 **Profiles 面板** 与右上 **Snap Matrix 面板** 之间切换焦点 |
| `↑` / `k` | 向上移动选择光标 |
| `↓` / `j` | 向下移动选择光标 |
| `Enter` | 在 Profiles 面板聚焦时：立即激活选中的 Profile |
| `s` | 无论焦点在何处：立即对当前活动窗口应用选中的 Snap 吸附布局 |
| `c` | 循环切换至下一个可用 Profile |
| `r` | 立即恢复基线，清除所有动态覆盖绑定 |
| `q` / `Esc` | 安全退出 TUI（不会影响后台守护进程与窗口状态） |

---

## 4. 卸载与清理

若需完全移除 AetherShift，运行提供的卸载脚本即可：
```bash
./scripts/uninstall.sh
```
该脚本会：
1. 自动执行 `aethershift restore` 注销所有 Hyprland 运行时绑定；
2. 停止并禁用 systemd 用户服务；
3. 清除 `~/.local/bin/` 二进制、systemd 服务单元和 Quickshell 软链接；
4. 清除运行时 socket 文件；
5. 保证用户的原生系统配置和桌面不受任何破坏。
