# 默认参数测试输入

[English](README.md) | **简体中文**

## 职责

配置测试使用的固定输入，描述待核对的配置定义／默认行为，不代表当前泰山派机器人的硬件配置。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [robotd-defaults.toml](robotd-defaults.toml) | 固定配置样例，包含不同串口与已启用策略 |
| [../robotd-params/src/lib.rs](../robotd-params/src/lib.rs) | 读取样例的配置定义／默认值测试 |
| [../robotctl/src/configure.rs](../robotctl/src/configure.rs) | 读取样例的编辑器往返／参数测试 |

## 调用与数据流

1. Rust 测试用 include_str! 嵌入此 TOML，核对解析、默认值与编辑行为。
2. 应用安装器选择 config/examples 或 config/board；测试输入不是实测机器人标定的来源。

## 边界与修改注意

- 不能因为测试把它称为 shipped／default 就复制到实机，它的 /dev/ttyS2 与启用策略状态不同于安装模板。
- 变更配置定义／默认值时有意识地更新预期，并同时检查参数库与编辑器两个使用方。

## 相关说明

- [详细使用说明](../../docs/zh-CN/configuration.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
