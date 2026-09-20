# WebPage_view 示例：Robot Panel

Harbor 风格的机器人控制面板示例。前端与 Harbor 相同：**Vue 3 + Vite + Tailwind CSS 4**，并直接使用 Harbor 的 `tokens.css`。Python 后端支持 ROS 2；当前环境无法导入 `rclpy` 时自动回退到 mock 数据。

## 开发

手动开发时，从 Task YAML 的 `webview_interface` 读取端口并导出环境变量，然后先起
Python、再起 Vite：

```bash
export HARBOR_WEBVIEW_INTERFACE_PORT="<webview_interface 中声明的端口>"
python3 -u robot_panel.py --harbor_info
npm install
npm run dev
```

浏览器打开 `http://127.0.0.1:18081/`。

## 给 Harbor 用

先构建静态页，再让 Python 托管 `dist/`：

```bash
npm install
npm run build
```

通过 Harbor 启动 Task 后，点击 Task 行上的面板按钮。Harbor 会创建独立 Panel 窗口并嵌入该页面；窗口标题由 Harbor 提供，页面内部不重复显示应用标题。

`--harbor_info` 会在服务就绪后向 stdout 输出一行诊断信息，方便手动确认监听地址：

```text
HARBOR_INFO {"kind":"webpage_view","title":"Robot Panel","url":"http://<lan-ip>:<panel-port>/","localhost_url":"http://127.0.0.1:<panel-port>/","bind":"0.0.0.0","port":<panel-port>}
```

Harbor 当前根据 Task YAML 的 `webview_interface` 生成面板地址，不依赖解析这行信息。

## 接入 ROS 2

先加载 ROS 2 和机器人 workspace，再启动面板：

```bash
source /opt/ros/humble/setup.bash
source ~/robot_ws/install/setup.bash
# 然后通过 Harbor 启动 robot-panel Task
```

只要当前 Python 能导入 `rclpy`，后端就会自动使用 ROS 2，并在日志中输出 `Robot Panel backend: ROS 2`；否则输出 `mock`。

面板订阅以下标准 topic：

- `/odom`：`nav_msgs/msg/Odometry`
- `/imu`：`sensor_msgs/msg/Imu`
- `/battery_state`：`sensor_msgs/msg/BatteryState`
- `/joint_states`：`sensor_msgs/msg/JointState`
- `/camera/color/compressed`：`sensor_msgs/msg/CompressedImage`，JPEG 数据通过 WebSocket 转发到网页

面板发布以下控制 topic：

- `/cmd_vel`：`geometry_msgs/msg/Twist`
- `/joint_commands`：`sensor_msgs/msg/JointState`
- `/robot_panel/mode`：`std_msgs/msg/String`
- `/robot_panel/estop`：`std_msgs/msg/Bool`

`/cmd_vel` 可直接对接常见移动底盘；关节、模式和急停 topic 需要机器人侧节点按自身控制器与安全策略进行转换。连接真实机器人前，应先确认速度限制、急停链路和控制权限。

视频通路使用与 HTTP 相同的面板端口，地址为 `/ws/video`。没有收到 ROS 相机帧时，后端会自动发送运动的 mock SVG 画面；收到 `/camera/color/compressed` 后会直接转发 JPEG 字节。前端负责断线重连、图片解码和 Canvas 等比显示。

## Harbor 配置

本示例自带 `harbor_taskcfg/tasks/robot-panel.yaml`，其中 `webview_interface` 是面板端口的唯一配置来源。Harbor 启动 Task 时会注入 `HARBOR_WEBVIEW_INTERFACE_PORT`；程序不再保存第二份端口。把本目录或 Harbor 仓库根加进 Search Paths 后会出现 Task `robot-panel`。启动任务，等它 running，点 Task 行上的网页图标即可打开。

前端只请求 `/api/state` 和 `/api/command`，ROS 2 逻辑集中在 `robot_panel.py` 的 `Ros2Bridge` 中。
