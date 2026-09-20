#!/usr/bin/env python3
import base64
import hashlib
import json
import math
import os
import socket
import struct
import sys
import threading
import time
from collections import deque
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse

try:
    import rclpy
    from geometry_msgs.msg import Twist
    from nav_msgs.msg import Odometry
    from rclpy.node import Node
    from rclpy.qos import qos_profile_sensor_data
    from sensor_msgs.msg import BatteryState, CompressedImage, Imu, JointState
    from std_msgs.msg import Bool, String

    ROS2_IMPORTABLE = True
except ImportError:
    ROS2_IMPORTABLE = False
    Node = object

def harbor_panel_port():
    raw_port = os.environ.get("HARBOR_WEBVIEW_INTERFACE_PORT")
    if raw_port is None:
        raise RuntimeError(
            "HARBOR_WEBVIEW_INTERFACE_PORT is missing; start this program through its Harbor Task"
        )
    try:
        port = int(raw_port)
    except ValueError as error:
        raise RuntimeError("HARBOR_WEBVIEW_INTERFACE_PORT must be an integer") from error
    if not 23000 <= port <= 24000:
        raise RuntimeError("HARBOR_WEBVIEW_INTERFACE_PORT must be between 23000 and 24000")
    return port


PORT = harbor_panel_port()
BIND = "127.0.0.1" if os.environ.get("HARBOR_WEBVIEW_LOCALHOST_ONLY") == "true" else "0.0.0.0"
TITLE = os.environ.get("HARBOR_WEBVIEW_NAME", "Robot Panel")
DIST = Path(__file__).resolve().parent / "dist"
HARBOR_INFO_FLAG = "--harbor_info"
ROS2_ENABLED = ROS2_IMPORTABLE

ROS_TOPICS = [
    {"name": "/camera/color/compressed", "type": "sensor_msgs/CompressedImage"},
    {"name": "/cmd_vel", "type": "geometry_msgs/Twist"},
    {"name": "/joint_states", "type": "sensor_msgs/JointState"},
    {"name": "/imu", "type": "sensor_msgs/Imu"},
    {"name": "/odom", "type": "nav_msgs/Odometry"},
    {"name": "/battery_state", "type": "sensor_msgs/BatteryState"},
]

WEBSOCKET_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def lan_ip():
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.connect(("8.8.8.8", 80))
        return sock.getsockname()[0]
    except OSError:
        return "127.0.0.1"
    finally:
        sock.close()


def quaternion_to_euler(x, y, z, w):
    sin_roll = 2.0 * (w * x + y * z)
    cos_roll = 1.0 - 2.0 * (x * x + y * y)
    roll = math.atan2(sin_roll, cos_roll)

    sin_pitch = 2.0 * (w * y - z * x)
    pitch = math.copysign(math.pi / 2.0, sin_pitch) if abs(sin_pitch) >= 1.0 else math.asin(sin_pitch)

    sin_yaw = 2.0 * (w * z + x * y)
    cos_yaw = 1.0 - 2.0 * (y * y + z * z)
    yaw = math.atan2(sin_yaw, cos_yaw)
    return roll, pitch, yaw


class VideoStream:
    def __init__(self):
        self.condition = threading.Condition()
        self.frame = b""
        self.opcode = 2
        self.sequence = 0
        self.last_ros_frame = 0.0

    def publish(self, frame, opcode, ros_frame=False):
        with self.condition:
            self.frame = frame
            self.opcode = opcode
            self.sequence += 1
            if ros_frame:
                self.last_ros_frame = time.time()
            self.condition.notify_all()

    def next_frame(self, sequence):
        with self.condition:
            self.condition.wait_for(lambda: self.sequence != sequence, timeout=2.0)
            return self.sequence, self.opcode, self.frame

    def needs_mock_frame(self):
        with self.condition:
            return time.time() - self.last_ros_frame > 1.0


video_stream = VideoStream()


def websocket_frame(payload, opcode):
    length = len(payload)
    if length < 126:
        header = struct.pack("!BB", 0x80 | opcode, length)
    elif length < 65536:
        header = struct.pack("!BBH", 0x80 | opcode, 126, length)
    else:
        header = struct.pack("!BBQ", 0x80 | opcode, 127, length)
    return header + payload


def mock_video_svg(elapsed):
    width = 640
    height = 360
    ball_x = 80 + int((math.sin(elapsed * 1.4) + 1.0) * 240)
    ball_y = 70 + int((math.cos(elapsed * 1.1) + 1.0) * 110)
    sweep_x = int((elapsed * 95) % width)
    return """<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360">
<defs><linearGradient id="bg" x2="0" y2="1"><stop stop-color="#20252b"/><stop offset="1" stop-color="#101318"/></linearGradient></defs>
<rect width="640" height="360" fill="url(#bg)"/>
<g stroke="#34404a" stroke-width="1" opacity=".7">
<path d="M0 60H640M0 120H640M0 180H640M0 240H640M0 300H640"/>
<path d="M80 0V360M160 0V360M240 0V360M320 0V360M400 0V360M480 0V360M560 0V360"/>
</g>
<rect x="%d" width="3" height="360" fill="#4ea1ff" opacity=".35"/>
<circle cx="%d" cy="%d" r="26" fill="#4ea1ff" opacity=".9"/>
<circle cx="%d" cy="%d" r="42" fill="none" stroke="#75b7ff" stroke-width="2" opacity=".45"/>
<text x="18" y="30" fill="#aeb8c2" font-family="monospace" font-size="14">HARBOR MOCK CAMERA</text>
<text x="18" y="338" fill="#6f7c88" font-family="monospace" font-size="12">websocket frame %06d</text>
</svg>""" % (sweep_x, ball_x, ball_y, ball_x, ball_y, int(elapsed * 10))


def loop_mock_video():
    started = time.time()
    while True:
        if video_stream.needs_mock_frame():
            frame = mock_video_svg(time.time() - started).encode("utf-8")
            video_stream.publish(frame, opcode=1)
        time.sleep(0.1)


class Robot:
    def __init__(self):
        self.lock = threading.Lock()
        self.backend = "mock"
        self.bridge = None
        self.last_ros_message = 0.0
        self.topic_activity = {item["name"]: deque(maxlen=30) for item in ROS_TOPICS}
        self.mode = "idle"
        self.estop = False
        self.battery = 87.4
        self.x = 0.0
        self.y = 0.0
        self.yaw = 0.0
        self.vx = 0.0
        self.wz = 0.0
        self.cmd_vx = 0.0
        self.cmd_wz = 0.0
        self.arm_enabled = True
        self.imu_roll = 0.0
        self.imu_pitch = 0.0
        self.imu_yaw = 0.0
        self.joints = [
            {"id": "shoulder_pan", "name": "shoulder pan", "rad": 0.12},
            {"id": "shoulder_lift", "name": "shoulder lift", "rad": -0.64},
            {"id": "elbow", "name": "elbow", "rad": 1.18},
            {"id": "wrist_flex", "name": "wrist flex", "rad": -0.42},
            {"id": "wrist_roll", "name": "wrist roll", "rad": 0.08},
            {"id": "gripper", "name": "gripper", "rad": 0.35},
        ]
        self.started = time.time()

    def attach_ros2(self, bridge):
        with self.lock:
            self.backend = "ros2"
            self.bridge = bridge

    def record_topic(self, topic, marks_connected=True):
        now = time.time()
        with self.lock:
            if marks_connected:
                self.last_ros_message = now
            self.topic_activity[topic].append(now)

    def update_odometry(self, message):
        orientation = message.pose.pose.orientation
        _, _, yaw = quaternion_to_euler(orientation.x, orientation.y, orientation.z, orientation.w)
        with self.lock:
            self.x = message.pose.pose.position.x
            self.y = message.pose.pose.position.y
            self.yaw = yaw
            self.vx = message.twist.twist.linear.x
            self.wz = message.twist.twist.angular.z
        self.record_topic("/odom")

    def update_imu(self, message):
        orientation = message.orientation
        roll, pitch, yaw = quaternion_to_euler(orientation.x, orientation.y, orientation.z, orientation.w)
        with self.lock:
            self.imu_roll = roll
            self.imu_pitch = pitch
            self.imu_yaw = yaw
        self.record_topic("/imu")

    def update_battery(self, message):
        percentage = float(message.percentage)
        if math.isfinite(percentage) and percentage >= 0.0:
            with self.lock:
                self.battery = percentage * 100.0 if percentage <= 1.0 else percentage
        self.record_topic("/battery_state")

    def update_joints(self, message):
        joints = []
        for name, position in zip(message.name, message.position):
            joints.append({"id": name, "name": name.replace("_", " "), "rad": float(position)})
        if joints:
            with self.lock:
                self.joints = joints
        self.record_topic("/joint_states")

    def update_camera(self, message):
        video_stream.publish(bytes(message.data), opcode=2, ros_frame=True)
        self.record_topic("/camera/color/compressed")

    def topic_rate(self, topic, now):
        samples = self.topic_activity[topic]
        if len(samples) < 2 or now - samples[-1] > 2.0:
            return 0.0
        duration = samples[-1] - samples[0]
        return 0.0 if duration <= 0.0 else (len(samples) - 1) / duration

    def snapshot(self):
        with self.lock:
            now = time.time()
            imu_t = now - self.started
            connected = self.backend == "mock" or now - self.last_ros_message < 3.0
            if self.backend == "mock":
                imu = {
                    "roll": round(0.02 * math.sin(imu_t), 3),
                    "pitch": round(-0.03 * math.cos(imu_t * 0.7), 3),
                    "yaw": round(self.yaw, 3),
                }
                topics = [
                    {"name": "/camera/color/compressed", "hz": 10.0, "type": "sensor_msgs/CompressedImage"},
                    {"name": "/cmd_vel", "hz": 12.4 if self.mode == "teleop" else 0.0, "type": "geometry_msgs/Twist"},
                    {"name": "/joint_states", "hz": 50.0, "type": "sensor_msgs/JointState"},
                    {"name": "/imu", "hz": 100.0, "type": "sensor_msgs/Imu"},
                    {"name": "/odom", "hz": 30.0, "type": "nav_msgs/Odometry"},
                    {"name": "/battery_state", "hz": 1.0, "type": "sensor_msgs/BatteryState"},
                ]
            else:
                imu = {
                    "roll": round(self.imu_roll, 3),
                    "pitch": round(self.imu_pitch, 3),
                    "yaw": round(self.imu_yaw, 3),
                }
                topics = [dict(item, hz=round(self.topic_rate(item["name"], now), 1)) for item in ROS_TOPICS]
            return {
                "title": TITLE,
                "robot": "harbor-bot",
                "backend": self.backend,
                "connected": connected,
                "mode": self.mode,
                "estop": self.estop,
                "arm_enabled": self.arm_enabled,
                "battery_percent": round(self.battery, 1),
                "uptime_s": int(now - self.started),
                "pose": {
                    "x": round(self.x, 3),
                    "y": round(self.y, 3),
                    "yaw": round(self.yaw, 3),
                },
                "velocity": {
                    "vx": round(self.vx, 3),
                    "wz": round(self.wz, 3),
                },
                "imu": imu,
                "joints": [dict(item) for item in self.joints],
                "camera": {
                    "topic": "/camera/color/compressed" if self.backend == "ros2" else "mock websocket",
                    "fps": 10.0 if self.backend == "mock" else round(self.topic_rate("/camera/color/compressed", now), 1),
                    "status": "streaming",
                },
                "topics": topics,
            }

    def command(self, payload):
        kind = payload.get("type")
        publish = None
        with self.lock:
            if kind == "estop":
                self.estop = bool(payload.get("value", True))
                self.cmd_vx = 0.0
                self.cmd_wz = 0.0
                if self.estop:
                    self.mode = "idle"
                publish = ("estop", self.estop)
            elif kind == "mode":
                mode = payload.get("value")
                if mode in ("idle", "teleop", "auto") and not self.estop:
                    self.mode = mode
                    self.cmd_vx = 0.0
                    self.cmd_wz = 0.0
                    publish = ("mode", mode)
            elif kind == "drive" and self.mode == "teleop" and not self.estop:
                self.cmd_vx = max(-1.0, min(1.0, float(payload.get("vx", 0.0))))
                self.cmd_wz = max(-1.0, min(1.0, float(payload.get("wz", 0.0))))
                publish = ("drive", self.cmd_vx * 0.6, self.cmd_wz * 0.9)
            elif kind == "arm":
                self.arm_enabled = bool(payload.get("value", True))
            elif kind == "joint" and self.arm_enabled and not self.estop:
                joint_id = payload.get("id")
                value = max(-2.4, min(2.4, float(payload.get("rad", 0.0))))
                for joint in self.joints:
                    if joint["id"] == joint_id:
                        joint["rad"] = value
                        publish = ("joint", joint_id, value)
                        break
            bridge = self.bridge
        if bridge and publish:
            bridge.publish_command(publish)
        return self.snapshot()

    def tick(self, dt):
        with self.lock:
            if self.backend != "mock":
                return
            if self.estop or self.mode == "idle":
                self.vx = 0.0
                self.wz = 0.0
            elif self.mode == "auto":
                self.vx = 0.22
                self.wz = 0.35
            else:
                self.vx = self.cmd_vx * 0.6
                self.wz = self.cmd_wz * 0.9
            self.x += self.vx * math.cos(self.yaw) * dt
            self.y += self.vx * math.sin(self.yaw) * dt
            self.yaw = (self.yaw + self.wz * dt + math.pi) % (2 * math.pi) - math.pi
            drain = 0.01 * dt
            if abs(self.vx) > 0.01 or abs(self.wz) > 0.01:
                drain = 0.04 * dt
            self.battery = max(12.0, self.battery - drain)


class Ros2Bridge(Node):
    def __init__(self, robot_state):
        super().__init__("harbor_robot_panel")
        self.robot_state = robot_state

        self.cmd_vel_publisher = self.create_publisher(Twist, "/cmd_vel", 10)
        self.joint_publisher = self.create_publisher(JointState, "/joint_commands", 10)
        self.mode_publisher = self.create_publisher(String, "/robot_panel/mode", 10)
        self.estop_publisher = self.create_publisher(Bool, "/robot_panel/estop", 10)

        self.create_subscription(Odometry, "/odom", robot_state.update_odometry, qos_profile_sensor_data)
        self.create_subscription(Imu, "/imu", robot_state.update_imu, qos_profile_sensor_data)
        self.create_subscription(BatteryState, "/battery_state", robot_state.update_battery, qos_profile_sensor_data)
        self.create_subscription(JointState, "/joint_states", robot_state.update_joints, qos_profile_sensor_data)
        self.create_subscription(
            CompressedImage,
            "/camera/color/compressed",
            robot_state.update_camera,
            qos_profile_sensor_data,
        )

    def publish_command(self, command):
        kind = command[0]
        if kind == "drive":
            message = Twist()
            message.linear.x = command[1]
            message.angular.z = command[2]
            self.cmd_vel_publisher.publish(message)
            self.robot_state.record_topic("/cmd_vel", marks_connected=False)
        elif kind == "joint":
            message = JointState()
            message.header.stamp = self.get_clock().now().to_msg()
            message.name = [command[1]]
            message.position = [command[2]]
            self.joint_publisher.publish(message)
        elif kind == "mode":
            self.mode_publisher.publish(String(data=command[1]))
            self.publish_command(("drive", 0.0, 0.0))
        elif kind == "estop":
            self.estop_publisher.publish(Bool(data=command[1]))
            self.publish_command(("drive", 0.0, 0.0))


robot = Robot()


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_OPTIONS(self):
        self.send_response(204)
        self._cors()
        self.end_headers()

    def do_GET(self):
        path = urlparse(self.path).path
        if path == "/ws/video":
            return self._video_websocket()
        if path == "/api/health":
            return self._json(200, {"ok": True, "title": TITLE, "port": PORT, "backend": robot.backend})
        if path == "/api/state":
            return self._json(200, robot.snapshot())
        self._static(path)

    def do_POST(self):
        path = urlparse(self.path).path
        if path != "/api/command":
            return self._json(404, {"error": "not found"})
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        try:
            payload = json.loads(raw.decode("utf-8") or "{}")
        except (ValueError, UnicodeDecodeError):
            return self._json(400, {"error": "invalid json"})
        if not isinstance(payload, dict):
            return self._json(400, {"error": "invalid json"})
        return self._json(200, robot.command(payload))

    def log_message(self, fmt, *args):
        sys.stdout.write("%s - %s\n" % (self.address_string(), fmt % args))
        sys.stdout.flush()

    def _cors(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")

    def _video_websocket(self):
        key = self.headers.get("Sec-WebSocket-Key")
        if self.headers.get("Upgrade", "").lower() != "websocket" or not key:
            return self._json(400, {"error": "websocket upgrade required"})

        digest = hashlib.sha1((key + WEBSOCKET_GUID).encode("ascii")).digest()
        accept = base64.b64encode(digest).decode("ascii")
        self.send_response(101, "Switching Protocols")
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()
        self.close_connection = True

        sequence = -1
        try:
            while True:
                sequence, opcode, frame = video_stream.next_frame(sequence)
                if frame:
                    self.wfile.write(websocket_frame(frame, opcode))
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, OSError):
            return

    def _json(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self._cors()
        self.end_headers()
        self.wfile.write(body)

    def _static(self, path):
        if path == "/":
            path = "/index.html"
        rel = path.lstrip("/")
        target = (DIST / rel).resolve()
        if not DIST.is_dir() or not str(target).startswith(str(DIST.resolve()) + os.sep) or not target.is_file():
            return self._json(404, {"error": "not found; run npm run build"})
        data = target.read_bytes()
        types = {
            ".html": "text/html; charset=utf-8",
            ".css": "text/css; charset=utf-8",
            ".js": "text/javascript; charset=utf-8",
            ".svg": "image/svg+xml",
            ".woff": "font/woff",
            ".woff2": "font/woff2",
        }
        self.send_response(200)
        self.send_header("Content-Type", types.get(target.suffix, "application/octet-stream"))
        self.send_header("Content-Length", str(len(data)))
        self._cors()
        self.end_headers()
        self.wfile.write(data)


def parse_args(argv):
    harbor_info = False
    for arg in argv:
        if arg == HARBOR_INFO_FLAG:
            harbor_info = True
        else:
            raise SystemExit("unknown argument: %s" % arg)
    return harbor_info


def emit_harbor_info():
    host = lan_ip()
    info = {
        "kind": "webpage_view",
        "title": TITLE,
        "url": "http://%s:%d/" % (host, PORT),
        "localhost_url": "http://127.0.0.1:%d/" % PORT,
        "bind": BIND,
        "port": PORT,
    }
    sys.stdout.write("HARBOR_INFO %s\n" % json.dumps(info, separators=(",", ":")))
    sys.stdout.flush()


def loop_ticks():
    last = time.time()
    while True:
        now = time.time()
        robot.tick(now - last)
        last = now
        time.sleep(0.05)


def start_ros2():
    if not ROS2_ENABLED:
        return None
    rclpy.init(args=None)
    bridge = Ros2Bridge(robot)
    robot.attach_ros2(bridge)
    threading.Thread(target=rclpy.spin, args=(bridge,), daemon=True).start()
    return bridge


def stop_ros2(bridge):
    if bridge is None:
        return
    bridge.destroy_node()
    if rclpy.ok():
        rclpy.shutdown()


def main():
    # --- 阶段 1：解析 Harbor 启动信息 ---
    harbor_info = parse_args(sys.argv[1:])

    # --- 阶段 2：启动机器人数据后端 ---
    ros2_bridge = start_ros2()
    threading.Thread(target=loop_ticks, daemon=True).start()
    threading.Thread(target=loop_mock_video, daemon=True).start()
    backend = "ROS 2" if ros2_bridge else "mock"
    sys.stdout.write("%s backend: %s\n" % (TITLE, backend))

    # --- 阶段 3：启动面板 HTTP 服务 ---
    server = ThreadingHTTPServer((BIND, PORT), Handler)
    listen = "http://127.0.0.1:%d/" % PORT
    lan = "http://%s:%d/" % (lan_ip(), PORT)
    sys.stdout.write("%s listening on %s  (lan %s)\n" % (TITLE, listen, lan))
    sys.stdout.flush()
    if harbor_info:
        emit_harbor_info()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        sys.stdout.write("stopping\n")
        sys.stdout.flush()
    finally:
        server.server_close()
        stop_ros2(ros2_bridge)


if __name__ == "__main__":
    main()
