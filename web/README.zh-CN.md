# 浏览器控制页与网关

[English](README.md) | **简体中文**

## 职责

提供用户控制页面，并把浏览器请求转换为 robotd IPC。Python 负责 HTTP、控制会话／心跳、状态采集与请求校验，JavaScript 负责页面交互；浏览器不生成关节轨迹，也不运行 ONNX。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [server.py](server.py) | RPC 客户端、Gateway、状态快照、WebSocket 心跳与 HTTP 接口 |
| [assets/index.html](assets/index.html) | 控制布局与导航入口 |
| [assets/app.js](assets/app.js) | 摇杆、按钮、键盘、输入清理和控制权界面 |
| [assets/style.css](assets/style.css) | 控制页外观与响应式布局 |
| [tests/test_gateway.py](tests/test_gateway.py) | 模拟主控与网关回归检查 |
| [tests/controls.test.cjs](tests/controls.test.cjs) | 模拟网络的浏览器交互检查 |
| [requirements.lock](requirements.lock) | 组件 Python 依赖说明 |

## 调用与数据流

1. 浏览器将交互请求发给 server.py；Gateway 管理当前页面连接，并经 `/run/robotd.sock` 转发控制。
2. robotd 状态和订阅数据整理成快照供界面显示；地址与导航辅助来自 deploy。
3. 页面交接经设置服务协调；接替硬件占用的是标定服务，不是网页网关。

## 子目录

| 目录 | 说明 |
| --- | --- |
| [assets](assets/README.zh-CN.md) | 控制页浏览器资源 |

## 边界与修改注意

- 控制权不等于账号认证。服务默认端口 9527，其接口应限制在可信网络使用。
- 网页按模型与状态提供行走、头部、坐起和捡物控制，拒绝翻滚／跳跃。当前界面按钮仍为中文，英文文档也需对应实际标签。

## 相关说明

- [详细使用说明](../docs/zh-CN/web-control.md)
- [文档索引](../docs/zh-CN/README.md)
- [仓库维护规则](../AGENTS.md)
