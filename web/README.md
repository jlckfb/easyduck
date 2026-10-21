# 网页控制

Python 3.9+ 标准库服务，默认端口 9527。服务通过 `/run/robotd.sock` 连接主控，不直接占用舵机串口。

板端由 `xgoduck-web.service` 启动，浏览器访问 `http://开发板IP:9527`。页面提供站姿、行走/起身、捡物及状态显示；上力和动作按钮会影响实物。

```sh
python3 server.py --help
python3 -m unittest discover -s tests -v
```

测试使用模拟 RPC。安装与启动步骤见[软件使用说明](../docs/软件使用说明.md)。
