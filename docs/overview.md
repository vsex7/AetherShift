# AetherShift: Architecture & Product Overview

**AetherShift** 是专为 **Omarchy 4.x (Quattro)** 和 **Hyprland** 设计的下一代动态桌面范式切换守护进程。它以纯 Rust 构建，致力于帮助从 Windows、macOS 迁移而来的用户在 Linux 动态平铺窗口管理器环境下快速重获原生的肌肉记忆。

---

## 1. 核心设计理念

传统的桌面定制通常依赖大量的 shell 脚本动态修改文本配置文件（如 `hyprland.conf`），再调用 `hyprctl reload` 重新加载。这种方式存在致命缺陷：
- **高延迟与闪烁**：全量重新解析文件并刷新 compositor 会造成明显的屏幕卡顿或闪烁；
- **配置侵入性**：直接改写用户磁盘文件容易造成配置损坏，故障后难以完整恢复基线；
- **缺乏运行时状态与冲突仲裁**：静态配置文件无法实时追踪覆盖绑定与原生绑定的层级关系。

**AetherShift 采用「全内存热切换」与「运行时虚拟层」作为第一性原理**：
1. **全内存热切换（< 5ms 目标）**：通过 Hyprland IPC 动态注册/注销按键绑定与调度窗口，无需重新加载磁盘配置。
2. **零侵入与干净回滚**：不写用户静态配置文件；退出守护进程或执行 `restore` 即可立即恢复初始环境。
3. **分层虚拟绑定引擎**：在内存中维护「原始系统基线」与「激活 Profile 覆盖层」，支持冲突检测、按键优先级仲裁与增量下发。
4. **单静态二进制、极简依赖**：由高效可靠的 Rust 编写，无需 Python、Node.js、QML 或 Lua 运行时支持。

---

## 2. 系统整体架构

AetherShift 采用 Client-Server 架构，各组件职责分明：

```
┌────────────────────────────────────────────────────────────────────────┐
│                              用户交互层                                 │
│                                                                        │
│   aethershift (CLI)   │   aethershift-tui (终端UI)   │  Quickshell 插件  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Unix Domain Socket (长度前缀 + JSON)
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       aethershift-daemon (守护进程)                     │
│                                                                        │
│   ┌───────────────────────┐              ┌─────────────────────────┐   │
│   │   Profile Engine      │              │   Binding Virtual Layer │   │
│   │   - 预设加载 (TOML)   │              │   - 冲突检测与优先级    │   │
│   │   - 内存运行时动态增删│              │   - 增量覆盖与回滚栈    │   │
│   └───────────┬───────────┘              └────────────┬────────────┘   │
│               │                                       │                │
│   ┌───────────▼───────────┐              ┌────────────▼────────────┐   │
│   │   Window Scheduler    │              │   Usage & Analytics     │   │
│   │   - 14 种几何吸附矩阵 │              │   - 动作频次监控        │   │
│   │   - 多显示器边界感知  │              │   - 智能人体工学推荐    │   │
│   └───────────┬───────────┘              └─────────────────────────┘   │
└───────────────┼────────────────────────────────────────────────────────┘
                │ Unix Domain Socket (/tmp/hypr/$HYPRLAND_INSTANCE_SIGNATURE/)
                ▼
      Hyprland Compositor (IPC Socket / Dispatchers)
```

### 核心模块职责

- **`aethershift-protocol`**：
  定义守护进程与客户端之间的强类型通讯协议，包括 `Request`/`Response` 结构、JSON Codec、14 种几何 Snap 布局、统计度量与系统状态模型。
- **`aethershift-hyprland`**：
  封装与 Hyprland Compositor 的底层异步交互，提供 IPC Socket 通讯、批量绑定下发、窗口几何动态计算与状态监听。
- **`aethershift-core`**：
  状态机与业务逻辑核心。包含 Profile 管理器、快捷键冲突仲裁器、窗口布局调度器以及使用行为度量系统。
- **`aethershift-daemon`**：
  主服务守护进程。监听 `/run/user/$UID/aethershift.sock`，管理后台任务、信号处理及全局状态同步。
- **`aethershift-cli`**：
  功能完备的命令行客户端。支持极速切换、循环预设、布局吸附、自定义编辑与统计导出。
- **`aethershift-tui`**：
  基于 Ratatui 构建的双面板交互式终端控制台。实时展现守护进程状态、Profile 列表、Snap 几何矩阵与智能建议。

---

## 3. 通讯协议规范

- **传输层**：Unix Domain Socket（UDS），默认路径位于：
  `/run/user/$UID/aethershift.sock`（亦可通过 `AETHERSHIFT_SOCKET` 环境变量或 `--socket` 参数覆盖）。
- **权限安全**：套接字严格限制为当前用户访问（0600）。
- **帧协议**：
  ```
  +-----------------------+------------------------------------------+
  | Length: 4 Bytes (BE)  | Payload: UTF-8 Encoded JSON String       |
  +-----------------------+------------------------------------------+
  ```
- **核心指令集**：
  - `status`: 查询守护进程在线状态、运行时间、激活 Profile 及覆盖层计数。
  - `list_profiles`: 枚举可用 Profile、描述、绑定的数量与激活状态。
  - `switch { profile, force }`: 切换指定 Profile，批量增量应用绑定覆盖。
  - `cycle`: 循环切换至下一个 Profile。
  - `restore`: 卸载所有覆盖层，干净回到系统初始状态。
  - `apply_layout { layout }`: 触发高精度窗口几何吸附（半屏、三分屏、居中浮动、最大化等）。
  - `get_stats` / `get_recommendations`: 检索热点快捷键统计与人体工学优化建议。
  - `profile_create` / `profile_bind` / `profile_unbind` / `profile_save` / `profile_delete`: 动态管理 Profile。
  - `shutdown`: 优雅停机并触发自动恢复。

---

## 4. 预设系统 (Presets)

AetherShift 原生附带四大开箱即用的预设，定义于 `~/.config/aethershift/presets/`：

1. **`windows.toml`**：
   - 为 Windows 迁移用户量身定制。
   - `ALT + F4` 关闭窗口；`SUPER + LEFT/RIGHT/UP/DOWN` 经典 Snap 贴边与最大化；`CTRL + SHIFT + ESC` 呼出任务管理器；`SUPER + E` 打开资源管理器；`SUPER + L` 锁屏。
2. **`macos.toml`**：
   - 贴合 macOS (Command 习惯) 肌肉记忆。
   - `SUPER + Q` / `SUPER + W` 退出与关闭；`SUPER + CTRL + F` 切换全屏；`SUPER + SPACE` 呼出应用启动器；`SUPER + TAB` / `SUPER + SHIFT + TAB` 轮转窗口。
3. **`hybrid.toml`**：
   - 融合平铺效率与习惯键位的高级模式。
   - 兼顾常规窗口操作与高阶平铺分屏操作。
4. **`native.toml`**：
   - 纯净原语模式，完全保留 Omarchy / Hyprland 的原生键位。

---

## 5. 优雅生命周期保证

- **系统启动**：由 systemd user service 随图形会话（`graphical-session.target`）自动拉起。
- **故障自愈**：守护进程异常退出时，systemd 负责延迟 2 秒自动恢复重启。
- **干净注销与退出**：无论是正常关闭、服务停止还是直接卸载，AetherShift 均会在终止前执行 `restore` 逻辑，完全注销所有临时注入的 Hyprland 绑定，绝对不留孤儿按键冲突。
