# USB / 2.4G 手柄支持

[English](../en/usb-gamepad.md) | **简体中文**

2.4G 手柄通过配套 USB 接收器接入开发板，不通过网页蓝牙扫描配对。当前验证过的接收器在手柄连接后呈现为 `045e:028e Microsoft X-Box 360 pad`，由 Linux `xpad` 驱动转换为标准输入事件，EasyDuck 的 `padd` 使用现有通用手柄接口读取。

仓库中的 `deploy/kernel/easyduck-gamepad.config` 是内核配置输入；SDK 负责使用它编译与内核版本匹配的 `xpad.ko`，并将内核模块及 depmod 索引放入镜像。应用安装器安装 `deploy/modules-load.d/easyduck-gamepad.conf`，使驱动随开机加载。只安装应用文件或只修改配置片段，不能代替内核模块构建。

指定 SDK 的 EasyDuck 构建入口会在打包前及最终镜像解包后执行 `deploy/kernel/check_gamepad.py`，检查内核配置、模块版本、自动加载规则和 USB 设备匹配索引；不完整则构建验收失败。该检查不加载驱动、不访问开发板、不发送动作。

烧录完整镜像后，将接收器插入 USB 主机口，打开手柄并切换到其 2.4G 模式。连接成功后应出现 `js0` 和 `event` 手柄输入设备，`padd` 日志应显示 Xbox 手柄已连接。这个接收器在手柄未连接时可能显示 `Receiver Update`，不能据此认定驱动缺失。

按键功能沿用 EasyDuck 当前定义；驱动支持不改变网页与手柄的控制权规则，也不代表实物运动已通过验收。镜像中的标定为新机待标定模板，刷机前应另外备份现有机器标定。
