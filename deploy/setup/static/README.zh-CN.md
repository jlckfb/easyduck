# 连接设置浏览器资源

[English](README.md) | **简体中文**

## 职责

登录、连接设置和服务导航的静态页面，显示需要认证的设置服务返回的结果，不直接调用 Linux 工具。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [login.html](login.html) | 登录表单与服务端会话流程 |
| [index.html](index.html) | Wi-Fi／蓝牙设置布局 |
| [setup.js](setup.js) | 网络／手柄操作与局部状态更新 |
| [setup.css](setup.css) | 主题与响应式布局 |
| [navigation.html](navigation.html) | 交接确认／进度页面 |
| [navigation.js](navigation.js) | 确认、CSRF 数据、状态检查与目标跳转 |

## 调用与数据流

1. first_setup.py 提供模板与需要认证的接口，connections.py 执行系统网络／蓝牙变更。
2. 导航请求进入 Navigator，由其协调本机服务与硬件交接，再返回目标页面。

## 边界与修改注意

- 界面确认必须对应真实后端状态，不能把猜测的连接结果当成成功。
- 切换 Wi-Fi 或服务占用可能中断访问／控制。调整外观时保留正在填写的数据，不删除认证与确认检查。

## 相关说明

- [详细使用说明](../../../docs/zh-CN/connection-setup.md)
- [文档索引](../../../docs/zh-CN/README.md)
- [仓库维护规则](../../../AGENTS.md)
