FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive \
    HARBOR_VNC_DESKTOP_SESSION=ubuntu

# --- 阶段 1：安装 Harbor 远端运行与可视化依赖 ---
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash ca-certificates curl dbus-x11 gnome-session gnome-session-flashback \
        gnome-shell gnome-shell-extension-ubuntu-dock gnome-terminal \
        iproute2 nautilus novnc openbox \
        openssh-server procps python3 sudo tigervnc-standalone-server \
        ubuntu-session ubuntu-settings util-linux websockify x11-apps \
        yaru-theme-gnome-shell \
    && rm -rf /var/lib/apt/lists/*

# --- 阶段 2：创建独立远端用户与 SSH 登录入口 ---
RUN useradd --create-home --shell /bin/bash harbor \
    && echo 'harbor:harbor' | chpasswd \
    && usermod -aG sudo harbor \
    && printf 'harbor ALL=(ALL) NOPASSWD:ALL\n' >/etc/sudoers.d/harbor \
    && chmod 440 /etc/sudoers.d/harbor \
    && mkdir -p /run/sshd /etc/ssh/sshd_config.d \
    && printf 'PasswordAuthentication yes\nPermitRootLogin no\n' >/etc/ssh/sshd_config.d/harbor-test.conf

# --- 阶段 3：安装容器内的远端测试 Workspace ---
COPY --chown=harbor:harbor workspace /home/harbor/workspace

EXPOSE 22 23682 29385

HEALTHCHECK --interval=2s --timeout=2s --start-period=5s --retries=20 \
    CMD bash -c 'exec 3<>/dev/tcp/127.0.0.1/22'

CMD ["bash", "-lc", "mkdir -p /run/dbus /run/systemd/seats /run/systemd/sessions /run/systemd/users; rm -f /run/dbus/pid; dbus-daemon --system --fork; /usr/lib/systemd/systemd-logind & exec /usr/sbin/sshd -D -e"]
