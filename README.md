# FCoreTuner

[![License: GPL v2](https://img.shields.io/badge/License-GPL_v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![CI](https://github.com/FFGG10-29/FCoreTuner/actions/workflows/ci.yml/badge.svg)](https://github.com/FFGG10-29/FCoreTuner/actions/workflows/ci.yml)

FCoreTuner 是一款现代、开源的 ECU 调校软件，支持 EpicEFI、Speeduino、rusEFI 及其他兼容 TS 格式 INI 的后市场发动机控制单元。

## 下载

| 平台 | 版本说明 |
|------|----------|
| **Linux** | AppImage / DEB / RPM |
| **Windows** | 安装包 (MSI/EXE) |
| **macOS** | DMG (ARM64 & Intel) |

![FCoreTuner 表格编辑器](docs/screenshots/table-editor.png)

## 说明

本项目目前仍处于早期开发阶段，欢迎社区贡献。我们希望这个项目成为一个开放、协作开发的调校工具。

## 功能特性

### 核心功能
- **跨平台**：支持 Windows、macOS 和 Linux
- **现代架构**：Rust 核心 + Tauri 桌面 UI
- **INI 定义兼容**：兼容标准 ECU INI 定义文件
- **实时数据**：实时传感器显示，支持可配置仪表盘
- **多显示器支持**：可将任意标签页弹出到独立窗口，支持双向同步

### 表格编辑
- **2D/3D 表格编辑器**：功能齐全的网格编辑器，支持键盘导航
- **3D 可视化**：基于 React Three Fiber 的曲面网格，支持轨道控制
- **实时光标**：跟随模式，带倒三角指示器和历史轨迹
- **编辑工具**：等值设置、增减、缩放、插值、平滑
- **重分箱**：更改轴区间，自动进行 Z 值插值
- **复制/粘贴**：表格数据的标准剪贴板操作
- **表格对比**：调校版本之间的并排差异视图
- **烧录到 ECU**：直接将修改写入 ECU 内存

### 仪表盘与仪表
- **TS 兼容仪表盘**：可导入现有 .dash 文件
- **9 种仪表类型**：模拟表盘、条形图（水平/垂直）、数字读数、扫描仪表、折线图、直方图、虚线柱
- **自定义布局**：拖拽式仪表定位
- **设计器模式**：可视化编辑仪表盘布局
- **仪表盘管理**：创建、复制、重命名、删除、导出仪表盘
- **3 个默认仪表盘**：基础、竞速和调校布局

### 自动调校（AutoTune）
- **实时自动调校**：基于 AFR 目标的实时燃油表推荐
- **表格选择器**：选择要自动调校的表格
- **热力图**：可视化单元格权重和变化幅度
- **单元格锁定**：锁定单元格以防止 AutoTune 修改
- **权限限制**：配置最大调整百分比
- **参考表格**：加载/保存参考 CSV 文件

### 数据记录
- **可配置采样率**：1Hz 到 100Hz 记录
- **日志回放**：播放/暂停、进度滑块、可变速度（0.25x-4x）
- **CSV 支持**：加载 FCoreTuner 或 TunerStudio 格式的日志
- **通道选择**：选择要显示的通道

### 诊断工具
- **齿形记录器**：曲轴/凸轮轴触发模式分析，带 RPM 检测
- **复合记录器**：多通道波形显示，带同步状态
- **CSV 导出**：导出诊断捕获数据以供分析

### 数据管理
- **CSV 导出/导入**：以 CSV 格式导出和导入调校数据
- **恢复默认值**：将所有值恢复为 INI 默认值
- **还原点**：创建、加载和管理调校备份
- **TunerStudio 导入**：导入现有 TunerStudio 项目

### 单位偏好
- **温度**：°C、°F 或 Kelvin
- **压力**：kPa、PSI、bar 或 inHg
- **AFR 显示**：AFR 或 Lambda（支持燃料类型选择）
- **速度**：km/h 或 mph

### 性能计算器
- **基于物理的功率计算**：根据加速数据计算轮上马力
- **扭矩曲线**：查看不同 RPM 下的估算扭矩
- **加速时间**：估算 0-100km/h 和四分之一英里时间
- **车辆参数**：配置车重、风阻系数、轮胎直径、齿比

### 项目管理
- **基于项目的工作流**：按车辆/ECU 组织调校
- **INI 仓库**：管理 ECU 定义文件，支持签名匹配
- **在线 INI 搜索**：从 Speeduino 和 rusEFI GitHub 仓库下载 INI 文件
- **签名不匹配检测**：ECU 与 INI 不匹配时自动检测

## 支持的 ECU

### 当前支持
- **Speeduino** - 完整支持 INI 定义文件和串行协议
- **rusEFI** - 完整支持 INI 定义文件和串行协议
- **EpicEFI** - 通过标准 INI 格式完整支持

### 兼容
- 任何使用标准 INI 定义格式的 ECU（MegaTune/TunerStudio 兼容）
- Megasquirt MS2/MS3（部分支持 - 串行协议开发中）

## 快速开始

### 环境要求

- **Rust 1.75+** - 通过 [rustup](https://rustup.rs) 安装
- **Node.js 20+** - 用于 Tauri 前端

### 构建与运行

```bash
# 克隆仓库
git clone https://github.com/FFGG10-29/FCoreTuner.git
cd FCoreTuner

# 安装前端依赖
cd crates/fcoretuner-app
npm install

# 开发模式运行
npm run tauri dev
```

### Windows 用户注意事项

```bash
# 如果提示找不到 link.exe，需要安装 Visual Studio Build Tools

# 1. 安装 Visual Studio Build Tools
# 下载地址：
# https://visualstudio.microsoft.com/downloads/
#
# 滚动到 "Tools for Visual Studio" → "Build Tools for Visual Studio"
# 安装时只勾选此工作负载：
#
#   ✔ Desktop development with C++
#

# 2. 确保 Rust 使用 MSVC 工具链
rustup default stable-x86_64-pc-windows-msvc

# 3. 重启终端使 PATH 生效

# 4. 构建 FCoreTuner（开发模式）
cd crates/fcoretuner-app
npm install
npm run tauri dev
```

### 生产构建

```bash
cd crates/fcoretuner-app
npm run tauri build
```

## 项目结构

```
fcoretuner/
├── crates/
│   ├── fcoretuner-core/    # 核心 Rust 库
│   │   ├── src/
│   │   │   ├── ini/        # INI 文件解析
│   │   │   ├── protocol/   # 串行通信
│   │   │   ├── ecu/        # ECU 内存模型
│   │   │   ├── datalog/    # 数据记录
│   │   │   ├── autotune/   # AutoTune 算法
│   │   │   ├── tune/       # 调校文件管理
│   │   │   ├── dash/       # 仪表盘格式解析
│   │   │   └── project/    # 项目与还原点
│   │   └── Cargo.toml
│   └── fcoretuner-app/     # Tauri 桌面应用
│       ├── src/            # React 前端 (TypeScript)
│       │   ├── components/
│       │   │   ├── dashboards/   # 仪表盘与仪表渲染
│       │   │   ├── tables/       # 2D/3D 表格编辑器
│       │   │   ├── dialogs/      # 模态对话框
│       │   │   ├── diagnostics/  # 齿形/复合记录器
│       │   │   └── tuner-ui/     # 主 UI 组件
│       │   └── utils/     # 单位转换、偏好设置
│       └── src-tauri/     # Tauri 后端 (Rust)
├── docs/                  # 文档和截图
├── scripts/               # 构建和开发脚本
└── Cargo.toml             # 工作空间根配置
```

## 开发

开发设置和指南请参阅 [CONTRIBUTING.md](CONTRIBUTING.md)。

### 运行测试

```bash
cargo test --workspace
```

### 运行代码检查

```bash
cargo clippy --workspace
```

## 许可证

本程序为自由软件；您可以根据自由软件基金会发布的 GNU 通用公共许可证第 2 版的条款重新分发和/或修改它。

详见 [LICENSE](LICENSE)。

## 贡献

欢迎贡献！提交 PR 前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 致谢

FCoreTuner 是一个独立的开源项目，与 EFI Analytics 无关联。
