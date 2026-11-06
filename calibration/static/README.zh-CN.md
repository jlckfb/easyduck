# 标定台浏览器前端

[English](README.md) | **简体中文**

## 职责

显示标定状态，将用户明确操作转换成后端命令。所有面板共用状态读取流程，3D 使用本地网格和依赖；硬件访问和最终校验仍由 Python 服务完成。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [index.html](index.html) | 工作台面板与控件 |
| [app.js](app.js) | 关节／整机／IMU 流程与 3D 衔接 |
| [state-feed.js](state-feed.js) | 共用状态轮询、命令状态串行化、不自动重试写入 |
| [servo-id.js](servo-id.js) | 舵机扫描／改号界面 |
| [calibration-file.js](calibration-file.js) | 导入 JSON 的形状／大小解析，后续由后端校验 |
| [appearance.js](appearance.js) | 主题保存与连接提示 |
| [style.css](style.css) | 工作台布局与外观 |
| [model](model) | 网格文件、映射与来源 |
| [vendor](vendor) | 本地 three.js 模块与原始许可 |

## 调用与数据流

1. 浏览器经 state-feed.js 读取 /api/state，并向 /api/command 提交明确操作。
2. server.py 校验并操作总线／IMU，返回快照更新面板与 3D；JavaScript 不直接占用硬件。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [model](model/README.zh-CN.md) | 标定 3D 网格 |

## 边界与修改注意

- 前端预检查不能替代后端校验，写入超时后应先核对实际状态再决定是否再次操作。
- 3D 是可视化，不是物理仿真。保留第三方代码／许可与网格来源；界面标签目前仍为中文。

## 相关说明

- [详细使用说明](../../docs/zh-CN/calibration.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
