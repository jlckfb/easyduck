# 网页控制

Python 3.9+ 标准库服务，默认端口 9527。服务通过 `/run/robotd.sock` 连接主控，不直接占用舵机串口。

板端由 `xgoduck-web.service` 启动，浏览器访问 `http://开发板IP:9527`。页面提供站姿、行走/起身、捡物及状态显示；上力和动作按钮会影响实物。

```sh
python3 server.py --help
python3 -m unittest discover -s tests -v
```

测试使用模拟 RPC。安装与启动步骤见[软件使用说明](../docs/软件使用说明.md)。

## 页面切换

页面顶部可切换到连接设置或实物标定台。入口使用当前主机的设置服务（默认端口 80，读取 `/etc/easyduck/setup.env` 中的 `EASYDUCK_SETUP_PORT`），无线访问时不会跳回 USB 地址。设置服务的安装见 [连接设置说明](../deploy/setup/README.md)。

离开控制页会释放网页控制连接。进入标定需确认机器人已放稳，随后停止正式控制并断力；标定退出会检查草稿、运动及力矩状态，释放串口后恢复正式服务，不会自动上力或运行策略。
