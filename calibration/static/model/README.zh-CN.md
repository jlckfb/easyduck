# 标定 3D 网格

[English](README.md) | **简体中文**

## 职责

浏览器工作台使用的参考几何。十五个 STL 部件与映射／来源元数据用于显示躯干、头部和肢体，不是 ONNX 模型或实测舵机标定。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [manifest.json](manifest.json) | 加载器使用的部件文件名、缩放与源信息 |
| [sources.json](sources.json) | 记录网格来源、许可与哈希证据 |
| [trunk_base.stl](trunk_base.stl) | 躯干参考网格 |
| [head_pitch.stl](head_pitch.stl) | 头部组件网格示例 |
| [../app.js](../app.js) | 清单／STL 加载与姿态更新 |
| [../vendor/STLLoader.js](../vendor/STLLoader.js) | 本地 STL 解析器 |

## 调用与数据流

1. app.js 获取清单并加载 STL 部件，再根据标定状态更新显示变换。
2. 关节方向／HOME 与 IMU 安装通过工作台状态影响姿态显示，不是通过修改网格文件生效。

## 边界与修改注意

- 变更几何时保留部件命名、缩放、哈希和署名，不覆盖第三方许可记录。
- 3D 姿态看起来合理不能证明实物轴向或活动范围安全，仍须独立核对机器人。

## 相关说明

- [详细使用说明](../../../docs/zh-CN/calibration.md)
- [文档索引](../../../docs/zh-CN/README.md)
- [仓库维护规则](../../../AGENTS.md)
