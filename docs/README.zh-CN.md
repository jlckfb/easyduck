# 双语项目文档

[English](README.md) | **简体中文**

## 职责

用相互对应的语言目录组织用户／开发者详细文档。目录／模块 README 解释本地职责与调用关系，本目录的指南负责详细操作、接口说明及验证边界。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [en/README.md](en/README.md) | 英文指南索引 |
| [zh-CN/README.md](zh-CN/README.md) | 简体中文指南索引 |
| [AGENTS.md](AGENTS.md) | 术语、双语更新规则与证据边界 |
| [en/imu-drivers.md](en/imu-drivers.md) | 驱动扩展与坐标／时间约定 |
| [en/models.md](en/models.md) | 模型观测／动作约定与来源 |

## 调用与数据流

1. 根目录和组件 README 引导读者进入所选语言的指南，英文／中文指南使用相同文件名。
2. tools/check_docs.py 检查本地引用与成对结构／示例，维护者仍需核对实现含义和翻译后的警告。

## 边界与修改注意

- 文档不能覆盖代码／配置，也不能证明已部署状态；源码行为、离线检查和实物验证要分开。
- 这里不放机器人执行脚本，示例归 examples，维护程序归 tools；原始许可记录保持权威。

## 相关说明

- [文档索引](zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
