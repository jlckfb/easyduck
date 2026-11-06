# 实物动作展示动图

[English](README.md) | **简体中文**

## 职责

保存两个根 README 展示的五张实物动图，本地存储让展示不依赖区域性第三方图床；文件保留原动画与画面比例。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [sit-stand.gif](sit-stand.gif) | 坐下与起立展示 |
| [head-control.gif](head-control.gif) | 头部控制展示 |
| [pickup.gif](pickup.gif) | 捡东西展示 |
| [walking.gif](walking.gif) | 行走展示 |
| [standing-balance.gif](standing-balance.gif) | 站立与平衡展示 |

## 调用与数据流

1. 两个根 README 嵌入同一组文件，分别提供替代文字和说明。
2. 展示尺寸由 README 表格单元格和等比图片尺寸控制；调整排版不需要重新编码 GIF。

## 边界与修改注意

- 替换文件时保留动画，并核对完整帧序列；不能仅为标题对齐而拉伸或裁掉画面。
- 动图展示的是所录行为，不等于所有配置／动作均已验收；运行代码、ONNX 与原始 MP4 不放在这里。

## 相关说明

- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
