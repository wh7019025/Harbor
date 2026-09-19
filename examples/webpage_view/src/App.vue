<script setup lang="ts">
import { Camera, Gamepad2, Layers3 } from "lucide-vue-next";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

type Mode = "idle" | "teleop" | "auto";

type RobotState = {
  backend: "mock" | "ros2";
  robot: string;
  connected: boolean;
  mode: Mode;
  estop: boolean;
  arm_enabled: boolean;
  battery_percent: number;
  uptime_s: number;
  pose: { x: number; y: number; yaw: number };
  velocity: { vx: number; wz: number };
  imu: { roll: number; pitch: number; yaw: number };
  joints: { id: string; name: string; rad: number }[];
  camera: { topic: string; fps: number; status: string };
  topics: { name: string; hz: number }[];
};

const modes: Mode[] = ["idle", "teleop", "auto"];
const driveKeys: Record<string, { vx: number; wz: number }> = {
  w: { vx: 1, wz: 0 },
  s: { vx: -1, wz: 0 },
  a: { vx: 0, wz: 1 },
  d: { vx: 0, wz: -1 },
  q: { vx: 0.4, wz: 1 },
  e: { vx: 0.4, wz: -1 },
};

const state = ref<RobotState | null>(null);
const keys = ref(new Set<string>());
const videoCanvas = ref<HTMLCanvasElement | null>(null);
const videoStatus = ref("connecting");
let pollTimer: number | null = null;
let videoSocket: WebSocket | null = null;
let videoReconnectTimer: number | null = null;
let receivedFrame = 0;
let renderedFrame = 0;

const driveLocked = computed(() => !state.value || state.value.estop || state.value.mode !== "teleop");

function fmt(value: number, digits: number) {
  return value.toFixed(digits);
}

function clock(seconds: number) {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function badgeClass(kind: "idle" | "run" | "warn" | "danger") {
  if (kind === "run") {
    return "bg-[color-mix(in_srgb,var(--running)_22%,transparent)] text-[var(--running)]";
  }
  if (kind === "warn") {
    return "bg-[color-mix(in_srgb,var(--warn)_22%,transparent)] text-[var(--warn)]";
  }
  if (kind === "danger") {
    return "bg-[color-mix(in_srgb,var(--danger)_22%,transparent)] text-[var(--danger)]";
  }
  return "bg-[color-mix(in_srgb,var(--faint)_18%,transparent)] text-[var(--faint)]";
}

function batteryBarClass(percent: number) {
  if (percent < 20) return "bg-[var(--danger)]";
  if (percent < 40) return "bg-[var(--warn)]";
  return "alt";
}

async function command(payload: Record<string, unknown>) {
  const response = await fetch("/api/command", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  state.value = (await response.json()) as RobotState;
}

async function poll() {
  try {
    state.value = (await fetch("/api/state").then((res) => res.json())) as RobotState;
  } catch {
    if (state.value) state.value = { ...state.value, connected: false };
  }
}

function sendDrive() {
  if (driveLocked.value) return;
  let vx = 0;
  let wz = 0;
  for (const key of keys.value) {
    const part = driveKeys[key];
    if (!part) continue;
    vx += part.vx;
    wz += part.wz;
  }
  void command({
    type: "drive",
    vx: Math.max(-1, Math.min(1, vx)),
    wz: Math.max(-1, Math.min(1, wz)),
  });
}

function holdDrive(key: string) {
  if (key === "stop") {
    keys.value = new Set();
    void command({ type: "drive", vx: 0, wz: 0 });
    return;
  }
  const next = new Set(keys.value);
  next.add(key);
  keys.value = next;
  sendDrive();
}

function releaseDrive(key: string) {
  if (key === "stop") return;
  const next = new Set(keys.value);
  next.delete(key);
  keys.value = next;
  sendDrive();
}

function onKeyDown(event: KeyboardEvent) {
  const key = event.key.toLowerCase();
  if (!driveKeys[key] || event.repeat) return;
  holdDrive(key);
}

function onKeyUp(event: KeyboardEvent) {
  const key = event.key.toLowerCase();
  if (!driveKeys[key]) return;
  releaseDrive(key);
}

function drawVideoFrame(data: string | ArrayBuffer) {
  const frame = ++receivedFrame;
  const blob = typeof data === "string"
    ? new Blob([data], { type: "image/svg+xml" })
    : new Blob([data], { type: "image/jpeg" });
  const url = URL.createObjectURL(blob);
  const image = new Image();
  image.onload = () => {
    URL.revokeObjectURL(url);
    if (frame < renderedFrame) return;
    const canvas = videoCanvas.value;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;

    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.floor(rect.width * dpr));
    const height = Math.max(1, Math.floor(rect.height * dpr));
    canvas.width = width;
    canvas.height = height;
    context.fillStyle = "#101318";
    context.fillRect(0, 0, width, height);

    const scale = Math.min(width / image.naturalWidth, height / image.naturalHeight);
    const drawWidth = image.naturalWidth * scale;
    const drawHeight = image.naturalHeight * scale;
    context.drawImage(image, (width - drawWidth) / 2, (height - drawHeight) / 2, drawWidth, drawHeight);
    renderedFrame = frame;
    videoStatus.value = "streaming";
  };
  image.onerror = () => {
    URL.revokeObjectURL(url);
    videoStatus.value = "decode error";
  };
  image.src = url;
}

function connectVideo() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const socket = new WebSocket(`${protocol}//${window.location.host}/ws/video`);
  socket.binaryType = "arraybuffer";
  videoSocket = socket;
  videoStatus.value = "connecting";
  socket.onmessage = (event: MessageEvent<string | ArrayBuffer>) => drawVideoFrame(event.data);
  socket.onclose = () => {
    if (videoSocket !== socket) return;
    videoSocket = null;
    videoStatus.value = "reconnecting";
    videoReconnectTimer = window.setTimeout(connectVideo, 1000);
  };
  socket.onerror = () => socket.close();
}

onMounted(() => {
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
  connectVideo();
  void poll();
  pollTimer = window.setInterval(() => {
    void poll();
  }, 200);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("keyup", onKeyUp);
  if (pollTimer != null) window.clearInterval(pollTimer);
  if (videoReconnectTimer != null) window.clearTimeout(videoReconnectTimer);
  videoSocket?.close();
  videoSocket = null;
});
</script>

<template>
  <div class="st-shell flex h-screen flex-col gap-2 overflow-auto px-3 py-2 md:overflow-hidden">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="flex items-center gap-2">
        <span class="kicker">mode</span>
        <button
          v-for="mode in modes"
          :key="mode"
          class="btn !px-2 !py-1"
          type="button"
          :class="state?.mode === mode ? 'bg-[var(--accent-soft)] shadow-[inset_2px_0_0_0_var(--accent)]' : ''"
          :disabled="!!state?.estop && mode !== 'idle'"
          @click="command({ type: 'mode', value: mode })"
        >
          {{ mode }}
        </button>
      </div>
      <div class="flex items-center gap-1.5">
        <span
          class="readout shrink-0 rounded px-1 py-0.5 text-[10px] uppercase tracking-wide"
          :class="badgeClass(state?.connected ? 'run' : 'danger')"
        >
          {{ state?.connected ? "connected" : "offline" }}
        </span>
        <span
          class="readout shrink-0 rounded px-1 py-0.5 text-[10px] uppercase tracking-wide"
          :class="badgeClass(state?.mode === 'idle' ? 'idle' : 'run')"
        >
          {{ state?.mode ?? "idle" }}
        </span>
        <span
          class="readout shrink-0 rounded px-1 py-0.5 text-[10px] uppercase tracking-wide"
          :class="badgeClass(state?.estop ? 'danger' : 'idle')"
        >
          {{ state?.estop ? "estop on" : "estop off" }}
        </span>
        <button
          class="btn btn-danger ml-1"
          type="button"
          :class="state?.estop ? 'bg-[rgba(229,115,106,0.22)] text-[#ffb3ac]' : ''"
          @click="command({ type: 'estop', value: !state?.estop })"
        >
          ESTOP
        </button>
      </div>
    </div>

    <div
      class="grid min-h-0 flex-1 grid-cols-1 gap-2 overflow-visible md:grid-cols-[14rem_minmax(0,1fr)_15rem] md:grid-rows-1 md:overflow-hidden xl:grid-cols-[16.5rem_minmax(0,1fr)_18rem]"
    >
      <div class="flex min-h-[22rem] flex-col gap-2 md:min-h-0">
        <section class="flex min-h-0 flex-col overflow-hidden rounded-md border border-[var(--line-soft)]">
          <div class="flex items-center justify-between border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
            <div class="flex items-center gap-1.5">
              <Gamepad2 class="h-3.5 w-3.5 text-[var(--accent)]" />
              <span class="kicker">chassis</span>
            </div>
            <span class="readout text-[10px] text-[var(--faint)]">
              {{ driveLocked ? "drive locked" : "WASD · QE" }}
            </span>
          </div>
          <div class="p-2">
            <div class="mx-auto grid w-[10.5rem] grid-cols-3 gap-1.5">
              <span />
              <button class="btn !px-0 !py-1 font-mono text-[11px]" type="button" :disabled="driveLocked" @mousedown="holdDrive('w')" @mouseup="releaseDrive('w')" @mouseleave="releaseDrive('w')">W</button>
              <span />
              <button class="btn !px-0 !py-1 font-mono text-[11px]" type="button" :disabled="driveLocked" @mousedown="holdDrive('a')" @mouseup="releaseDrive('a')" @mouseleave="releaseDrive('a')">A</button>
              <button class="btn !px-0 !py-1 font-mono text-[11px]" type="button" :disabled="driveLocked" @mousedown="holdDrive('s')" @mouseup="releaseDrive('s')" @mouseleave="releaseDrive('s')">S</button>
              <button class="btn !px-0 !py-1 font-mono text-[11px]" type="button" :disabled="driveLocked" @mousedown="holdDrive('d')" @mouseup="releaseDrive('d')" @mouseleave="releaseDrive('d')">D</button>
              <button class="btn !px-0 !py-1 font-mono text-[11px]" type="button" :disabled="driveLocked" @mousedown="holdDrive('q')" @mouseup="releaseDrive('q')" @mouseleave="releaseDrive('q')">Q</button>
              <button class="btn !px-0 !py-1 font-mono text-[11px]" type="button" :disabled="driveLocked" @click="holdDrive('stop')">stop</button>
              <button class="btn !px-0 !py-1 font-mono text-[11px]" type="button" :disabled="driveLocked" @mousedown="holdDrive('e')" @mouseup="releaseDrive('e')" @mouseleave="releaseDrive('e')">E</button>
            </div>
            <p class="readout mt-2 text-center text-[11px] text-[var(--muted)]">
              vx {{ fmt(state?.velocity.vx ?? 0, 3) }} · wz {{ fmt(state?.velocity.wz ?? 0, 3) }}
            </p>
          </div>
        </section>
        <div class="grid grid-cols-2 gap-2">
          <article class="metric-tile">
            <div class="kicker">battery</div>
            <div class="readout mt-1.5 text-[13px] text-[var(--ink-bright)]">{{ fmt(state?.battery_percent ?? 0, 1) }}%</div>
            <div class="bar mt-2">
              <span
                :class="batteryBarClass(state?.battery_percent ?? 0)"
                :style="{ width: `${Math.max(0, Math.min(100, state?.battery_percent ?? 0))}%` }"
              />
            </div>
          </article>
          <article class="metric-tile">
            <div class="kicker">pose</div>
            <div class="readout mt-1.5 text-[11px] text-[var(--ink-bright)]">
              x {{ fmt(state?.pose.x ?? 0, 2) }} · y {{ fmt(state?.pose.y ?? 0, 2) }}
            </div>
            <div class="readout mt-1 text-[11px] text-[var(--muted)]">yaw {{ fmt(state?.pose.yaw ?? 0, 2) }}</div>
          </article>
          <article class="metric-tile">
            <div class="kicker">imu</div>
            <div class="readout mt-1.5 text-[11px] text-[var(--ink-bright)]">
              {{ fmt(state?.imu.roll ?? 0, 2) }}
              {{ fmt(state?.imu.pitch ?? 0, 2) }}
              {{ fmt(state?.imu.yaw ?? 0, 2) }}
            </div>
          </article>
          <article class="metric-tile">
            <div class="kicker">uptime</div>
            <div class="readout mt-1.5 text-[13px] text-[var(--ink-bright)]">{{ clock(state?.uptime_s ?? 0) }}</div>
          </article>
        </div>
      </div>

      <div class="grid min-h-[30rem] grid-rows-[minmax(0,1fr)_11rem] gap-2 md:min-h-0">
        <section class="flex min-h-0 flex-col overflow-hidden rounded-md border border-[var(--line-soft)]">
          <div class="flex items-center justify-between border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
            <div class="flex items-center gap-1.5">
              <Camera class="h-3.5 w-3.5 text-[var(--accent)]" />
              <span class="kicker">camera</span>
            </div>
            <span class="readout truncate text-[10px] text-[var(--faint)]">
              {{ state?.camera.topic ?? "websocket" }} · {{ videoStatus }}
            </span>
          </div>
          <div class="relative min-h-0 flex-1 bg-[var(--surface-2)]">
            <canvas ref="videoCanvas" class="block h-full w-full" />
          </div>
        </section>
        <section class="flex min-h-0 flex-col overflow-hidden rounded-md border border-[var(--line-soft)]">
          <div class="flex items-center justify-between border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
            <div class="flex items-center gap-1.5">
              <Layers3 class="h-3.5 w-3.5 text-[var(--accent)]" />
              <span class="kicker">topics</span>
            </div>
            <span class="readout text-[10px] text-[var(--faint)]">{{ state?.backend ?? "mock" }}</span>
          </div>
          <div class="min-h-0 flex-1 overflow-auto">
            <div
              v-for="topic in state?.topics ?? []"
              :key="topic.name"
              class="flex items-center justify-between gap-2 border-b border-[var(--line-soft)] px-2 py-1.5"
            >
              <span class="readout truncate text-[11px] text-[var(--ink)]">{{ topic.name }}</span>
              <span class="readout shrink-0 text-[11px] text-[var(--faint)]">{{ fmt(topic.hz, 1) }} hz</span>
            </div>
          </div>
        </section>
      </div>

      <section class="flex min-h-[20rem] flex-col overflow-hidden rounded-md border border-[var(--line-soft)] md:min-h-0">
        <div class="flex items-center justify-between border-b border-[var(--line-soft)] bg-[var(--bg-1)] px-2 py-1">
          <span class="kicker">arm</span>
          <button
            class="btn !px-2 !py-1 text-[11px]"
            type="button"
            @click="command({ type: 'arm', value: !state?.arm_enabled })"
          >
            {{ state?.arm_enabled ? "enabled" : "disabled" }}
          </button>
        </div>
        <div class="min-h-0 flex-1 space-y-3 overflow-auto p-2">
          <label v-for="joint in state?.joints ?? []" :key="joint.id" class="block">
            <span class="flex items-center justify-between gap-2">
              <span class="kicker">{{ joint.name }}</span>
              <span class="readout text-[11px] text-[var(--muted)]">{{ fmt(joint.rad, 2) }}</span>
            </span>
            <input
              class="mt-1.5 w-full accent-[var(--accent)]"
              type="range"
              min="-2.4"
              max="2.4"
              step="0.01"
              :value="joint.rad"
              :disabled="!state?.arm_enabled || !!state?.estop"
              @input="command({ type: 'joint', id: joint.id, rad: Number(($event.target as HTMLInputElement).value) })"
            />
          </label>
        </div>
      </section>
    </div>
  </div>
</template>
