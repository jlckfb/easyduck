# robotd-params — 共用配置定义

[English](README.md) | **简体中文**

## 职责

负责启动参数类型、默认值、校验、模型路径解析和可编辑字段元数据。robotd 读取配置，robotctl configure 按同一套定义编辑配置，避免维护两套不同的默认值和规则。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [src/lib.rs](src/lib.rs) | Params、IMU／策略／安全／总线／音频设置与校验 |
| [src/registry.rs](src/registry.rs) | 可编辑字段类型、单位、标签与查询 |
| [../robotd/src/params.rs](../robotd/src/params.rs) | 主控使用的重新导出入口 |
| [../robotctl/src/configure.rs](../robotctl/src/configure.rs) | 使用配置定义与登记表的 TOML 编辑器 |
| [../../config/examples/robotd.toml](../../config/examples/robotd.toml) | 公开格式样例 |

## 调用与数据流

1. robotd 加载并校验启动配置，再创建运行对象。
2. robotctl configure 使用登记表生成编辑界面，保存前调用共用定义校验。
3. Python 标定程序也读取 robotd.toml；修改 IMU 配置时必须保持两端兼容。

### 工作区直接关系

- 依赖的本地库：[duck-ipc-proto](../duck-ipc-proto/README.zh-CN.md)
- 直接使用方：[robotctl](../robotctl/README.zh-CN.md)、[robotd](../robotd/README.zh-CN.md)

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- 默认值变化主要影响新配置或省略字段，不一定改变已有机器的显式设置。修改公开样例时不能覆盖私有标定。
- 参数或可选子系统被定义，不代表当前 XGO 路径已实现。修改时同步配置类型、编辑元数据、模板和相关调用方。

## 相关说明

- [详细使用说明](../../docs/zh-CN/configuration.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
