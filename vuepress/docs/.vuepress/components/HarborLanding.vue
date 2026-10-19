<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";

// --- 阶段 1：真实采集视频的章节与播放状态 ---
const video = ref(null);
const time = ref(0);
const playing = ref(false);
const reduced = ref(false);
const failed = ref(false);
const chapters = [
  {
    time: 0,
    title: "启动一组任务",
    text: "一次启动 Hello Harbor，运行日志程序与 Robot Panel。",
    tag: "GROUP → TASKS",
  },
  {
    time: 2.24,
    title: "查看实时日志",
    text: "任务状态与实际输出并排显示，日志持续更新。",
    tag: "STATUS + LOGS",
  },
  {
    time: 9,
    title: "打开 Robot Panel",
    text: "从 Harbor 进入 Robot Panel，查看模拟相机与实时指标。",
    tag: "TASK → WEBVIEW",
  },
];
// 首页能力说明对应当前文档入口；演示视频只覆盖任务、日志与 WebView。
const features = [
  {
    label: "01 / TASK & GROUP",
    title: "任务配置与批量启动",
    description:
      "保存命令、工作目录和多套运行配置。单独启动、停止或重启任务，也可按 Group 定义的顺序运行多个程序。",
    href: "/guide/tasks/",
    link: "查看任务配置",
  },
  {
    label: "02 / REMOTE",
    title: "远端终端与桌面",
    description:
      "通过 SSH 连接远端 Linux 主机，打开内置终端。查看已登录的 X11 桌面，或在共享虚拟桌面中操作 Qt、GTK 等原生窗口。",
    href: "/guide/remote/",
    link: "查看远端连接方式",
  },
  {
    label: "03 / WEBVIEW",
    title: "打开程序面板",
    description:
      "为任务接入一个或多个 Web 页面，在 Harbor 中查看相机画面、状态数据，或使用程序提供的控制界面。",
    href: "/guide/panels/",
    link: "接入程序界面",
  },
  {
    label: "04 / MOBILE",
    title: "手机扫码访问面板",
    description:
      "开启 Mobile 后，手机扫码打开正在运行、允许网络访问的 WebView。手机与运行面板的主机需要网络互通。",
    href: "/guide/panels/#手机访问面板",
    link: "设置手机访问",
  },
  {
    label: "05 / LOGS & PROCESSES",
    title: "日志、资源指标与进程",
    description:
      "查看实时输出与历史日志、CPU 和内存等资源指标。任务管理器显示进程组、子进程和残留进程。",
    href: "/guide/logs/",
    link: "查看运行与排查功能",
  },
  {
    label: "06 / AI & API",
    title: "让 AI 配置和操作任务",
    description:
      "向支持 Skill 的 AI 提供 Harbor Tag，指定要操作的任务。AI 可通过 Core API 修改配置、执行任务、读取状态和日志。",
    href: "/guide/work-with-ai/",
    link: "使用 Harbor Skill",
  },
];
const chapter = computed(() =>
  time.value >= 9 ? 2 : time.value >= 2.24 ? 1 : 0,
);
async function playAt(seconds) {
  if (!video.value) return;
  if (seconds !== undefined) video.value.currentTime = seconds;
  try {
    await video.value.play();
  } catch {
    playing.value = false;
  }
}
function toggle() {
  playing.value ? video.value?.pause() : playAt();
}
function motionChanged(event) {
  reduced.value = event.matches;
  if (event.matches) video.value?.pause();
}
function visibilityChanged() {
  if (document.hidden) video.value?.pause();
}
let motion;
// --- 阶段 2：尊重减少动态效果偏好，清理浏览器监听 ---
onMounted(() => {
  motion = window.matchMedia("(prefers-reduced-motion: reduce)");
  motionChanged(motion);
  motion.addEventListener("change", motionChanged);
  document.addEventListener("visibilitychange", visibilityChanged);
  if (!reduced.value) playAt();
});
onUnmounted(() => {
  motion?.removeEventListener("change", motionChanged);
  document.removeEventListener("visibilitychange", visibilityChanged);
});
</script>

<template>
  <main class="harbor-landing">
    <div class="hero-stage">
      <section class="hero">
        <div class="eyebrow"><i></i> Harbor · Every task, ready to sail.</div>
        <div class="hero-row">
          <h1>程序、终端与桌面，<br /><span>集中管理与操作。</span></h1>
          <div class="hero-copy">
            <p>
              运行任务、查看日志，打开远端终端与桌面。<br />支持程序面板、手机访问和
              AI 操作。
            </p>
            <div class="actions">
              <a
                class="primary"
                href="https://github.com/wh7019025/Harbor/releases"
                >下载 Harbor <span>↗</span></a
              ><a href="/guide/getting-started/">快速开始 <span>→</span></a>
            </div>
            <div class="meta">
              Linux 本机运行 · Windows / macOS / Linux 远端连接
            </div>
          </div>
        </div>
      </section>

      <section id="demo" class="demo" aria-label="Harbor 实际运行演示">
        <div class="demo-top">
          <span><i></i> 任务与面板演示</span>
        </div>
        <div class="theater">
          <div class="screen">
            <video
              ref="video"
              muted
              playsinline
              loop
              controls
              preload="metadata"
              poster="/media/harbor-workflow-poster.jpg"
              aria-label="Harbor 启动任务组、查看日志、打开 Robot Panel 的真实运行画面"
              @timeupdate="time = $event.target.currentTime"
              @play="playing = true"
              @pause="playing = false"
              @error="failed = true"
            >
              <source
                src="/media/harbor-workflow.mp4?v=click-cues-1"
                type="video/mp4"
              />
              <track
                kind="captions"
                src="/media/harbor-workflow.vtt"
                srclang="zh"
                label="中文"
              />
            </video>
            <p v-if="failed" class="fallback">
              视频暂时无法播放。<a
                href="/media/harbor-workflow.mp4?v=click-cues-1"
                >打开演示视频 ↗</a
              >
            </p>
          </div>
          <div class="story">
            <div class="story-heading">
              启动任务组。<br /><span>查看日志和程序面板。</span>
            </div>
            <button
              v-for="(item, index) in chapters"
              :key="item.title"
              class="chapter"
              :class="{ active: chapter === index }"
              :aria-pressed="chapter === index"
              @click="playAt(item.time)"
            >
              <span class="number">0{{ index + 1 }}</span
              ><span class="chapter-copy"
                ><small>{{ item.tag }}</small
                ><strong>{{ item.title }}</strong
                ><span>{{ item.text }}</span></span
              >
            </button>
            <button class="play-control" @click="toggle">
              {{ playing ? "Ⅱ 暂停演示" : "▶ 播放演示"
              }}<span
                >{{ Math.floor(time).toString().padStart(2, "0") }} / 22 s</span
              >
            </button>
          </div>
        </div>
        <div class="demo-note">
          <span>原版 Harbor 前端 + 真实 Core 运行采集，片段经剪辑。</span
          ><span>Robot Panel 使用示例模拟数据。</span>
        </div>
      </section>
    </div>

    <section class="workflow wrap">
      <div class="section-label">Harbor 功能</div>
      <div class="section-heading">
        <h2>运行任务、连接远端，<br />操作终端与程序界面。</h2>
        <p>
          桌面端管理程序与机器连接。<br />手机打开运行中的面板。<br />AI 通过
          API 操作任务。
        </p>
      </div>
      <div class="feature-grid">
        <a
          v-for="feature in features"
          :key="feature.title"
          class="feature"
          :href="feature.href"
        >
          <span class="feature-index">{{ feature.label }}</span>
          <h3>{{ feature.title }}</h3>
          <p>{{ feature.description }}</p>
          <span class="feature-link">{{ feature.link }} →</span>
        </a>
      </div>
    </section>
    <section class="closing wrap">
      <div>
        <span class="section-label">开始使用 Harbor</span>
        <h2>安装 Harbor，添加第一个任务。</h2>
      </div>
      <a class="primary" href="https://github.com/wh7019025/Harbor/releases"
        >获取 Harbor <span>↗</span></a
      >
    </section>
  </main>
</template>

<style scoped>
/* --- 阶段 3：沿用 src/styles/tokens.css 的应用色彩与控件语言 --- */
.harbor-landing {
  --bg-0: #2a2a2a;
  --bg-1: #333333;
  --surface: #3a3a3a;
  --surface-hover: #454545;
  --ink: #d8d8d8;
  --ink-bright: #f0f0f0;
  --muted: #a8a8a8;
  --faint: #7a7a7a;
  --line: #4a4a4a;
  --line-soft: #3c3c3c;
  --accent: #4ea1ff;
  --accent-dim: #2b7fd4;
  --accent-soft: rgba(78, 161, 255, 0.16);
  background: var(--bg-0);
  color: var(--ink);
  font-family: "Segoe UI", "Ubuntu", "Helvetica Neue", sans-serif;
  overflow: hidden;
}
.wrap {
  max-width: 1280px;
  margin: 0 auto;
  padding-inline: 48px;
}
a {
  color: inherit;
  text-decoration: none !important;
}
.hero {
  padding-top: 72px;
  padding-bottom: 52px;
  text-align: center;
}
.eyebrow,
.section-label {
  color: var(--muted);
  font-size: 12px;
  letter-spacing: 0.08em;
}
.eyebrow {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
}
.eyebrow i,
.demo-top i {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--accent);
}
.hero-row {
  margin-top: 25px;
}
h1 {
  color: var(--ink-bright);
  font-size: clamp(42px, 5.5vw, 72px);
  font-weight: 600;
  line-height: 1.2;
  letter-spacing: -0.045em;
  margin: 0;
}
h1 br {
  display: none;
}
h1 span {
  color: var(--ink-bright);
}
.hero-copy {
  margin-top: 24px;
}
.hero-copy p {
  font-size: 17px;
  line-height: 1.8;
  color: var(--muted);
  margin: 0;
}
.hero-copy p br {
  display: none;
}
.actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  margin-top: 28px;
}
.primary,
.actions > a {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 28px;
  min-height: 46px;
  padding: 10px 23px;
  border-radius: 6px;
  border: 1px solid var(--line);
  background: var(--surface);
  color: var(--ink-bright);
  font-size: 14px;
  font-weight: 500;
  transition:
    background 160ms,
    border-color 160ms;
}
.primary,
.actions > .primary {
  background: var(--accent-dim);
  border-color: transparent;
  color: white;
}
.primary:hover,
.actions > .primary:hover {
  background: #1177bb;
}
.actions > a:hover {
  background: var(--surface-hover);
}
.meta {
  color: var(--faint);
  font-size: 12px;
  margin-top: 19px;
}
.demo {
  scroll-margin-top: 85px;
}
.demo-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--muted);
  font-size: 12px;
  margin-bottom: 14px;
}
.demo-top i {
  margin-right: 9px;
}
.theater {
  overflow: hidden;
  background: var(--bg-1);
  border: 1px solid var(--line);
  border-radius: 8px;
  box-shadow: 0 24px 60px #0003;
}
.screen {
  background: var(--bg-0);
}
video {
  display: block;
  width: 100%;
  aspect-ratio: 1.6;
  object-fit: contain;
}
.story {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  border-top: 1px solid var(--line);
}
.story-heading {
  display: none;
}
.chapter {
  position: relative;
  font: inherit;
  display: flex;
  gap: 14px;
  align-items: flex-start;
  padding: 24px;
  text-align: left;
  border: 0;
  border-right: 1px solid var(--line-soft);
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  transition: background 160ms;
}
.chapter:nth-of-type(3) {
  border-right: 0;
}
.chapter:hover {
  background: var(--surface);
}
.chapter.active {
  background: var(--accent-soft);
  color: var(--ink-bright);
}
.chapter.active::before {
  content: "";
  position: absolute;
  height: 2px;
  top: 0;
  left: 0;
  right: 0;
  background: var(--accent);
}
.number {
  font:
    12px "Cascadia Code",
    "Consolas",
    monospace;
  color: var(--faint);
  padding-top: 4px;
}
.chapter.active .number {
  color: var(--accent);
}
.chapter-copy {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.chapter small {
  display: none;
}
.chapter strong {
  font-size: 16px;
  font-weight: 500;
}
.chapter-copy > span {
  font-size: 12px;
  line-height: 1.75;
  color: var(--muted);
}
.play-control {
  grid-column: 1/-1;
  display: flex;
  justify-content: space-between;
  background: var(--bg-0);
  border: 0;
  border-top: 1px solid var(--line-soft);
  padding: 12px 24px;
  color: var(--muted);
  font-size: 12px;
  cursor: pointer;
}
.play-control:hover {
  color: var(--ink-bright);
}
.play-control span {
  font-family: "Cascadia Code", "Consolas", monospace;
}
.demo-note {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-top: 16px;
  color: var(--faint);
  font-size: 11px;
  line-height: 1.7;
}
.fallback {
  padding: 20px;
}
.workflow {
  padding-top: 112px;
  padding-bottom: 96px;
}
.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 22px 0 42px;
  gap: 40px;
}
h2 {
  color: var(--ink-bright);
  font-size: 40px;
  line-height: 1.35;
  font-weight: 500;
  letter-spacing: -0.025em;
  border: 0;
  padding: 0;
  margin: 0;
}
.section-heading p {
  color: var(--muted);
  font-size: 15px;
  line-height: 1.9;
  margin: 0;
}
.feature-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  border-top: 1px solid var(--line);
  border-bottom: 1px solid var(--line);
}
.feature {
  padding: 32px;
  border-right: 1px solid var(--line-soft);
  border-bottom: 1px solid var(--line-soft);
  transition: background 160ms;
}
.feature:nth-child(3n + 1) {
  padding-left: 0;
}
.feature:nth-child(3n) {
  border-right: 0;
}
.feature:hover {
  background: var(--bg-1);
}
.feature-index {
  color: var(--faint);
  font-size: 11px;
  font-family: "Cascadia Code", "Consolas", monospace;
}
.feature h3 {
  color: var(--ink-bright);
  font-size: 21px;
  font-weight: 500;
  line-height: 1.5;
  margin: 20px 0 14px;
}
.feature p {
  font-size: 14px;
  line-height: 1.9;
  color: var(--muted);
  margin: 0;
}
.feature-link {
  display: block;
  font-size: 13px;
  color: var(--accent);
  margin-top: 28px;
}
.config-art,
.connection-art,
.bars {
  display: none;
}
.closing {
  border-top: 1px solid var(--line-soft);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 32px;
  padding-top: 52px;
  padding-bottom: 64px;
}
.closing h2 {
  font-size: 32px;
  margin-top: 16px;
}
button:focus-visible,
a:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 4px;
}
@media (max-width: 900px) {
  .wrap {
    padding-inline: 28px;
  }
  .hero {
    padding-top: 56px;
  }
  h1 {
    font-size: 52px;
  }
  h1 br {
    display: block;
  }
  .hero-copy p br {
    display: block;
  }
  .chapter {
    padding: 20px 16px;
    gap: 9px;
  }
  .chapter strong {
    font-size: 14px;
  }
  .feature {
    padding: 24px;
  }
  .feature h3 {
    font-size: 18px;
  }
  h2 {
    font-size: 34px;
  }
}
@media (max-width: 600px) {
  .wrap {
    padding-inline: 20px;
  }
  .hero {
    padding-top: 44px;
    padding-bottom: 36px;
  }
  .eyebrow {
    font-size: 11px;
  }
  h1 {
    font-size: 43px;
    line-height: 1.25;
  }
  .hero-copy p {
    font-size: 14px;
  }
  .meta {
    font-size: 10px;
  }
  .demo-top {
    font-size: 10px;
  }
  .story {
    grid-template-columns: 1fr;
  }
  .chapter {
    padding: 16px;
    border-right: 0;
    border-bottom: 1px solid var(--line-soft);
  }
  .chapter.active::before {
    width: 2px;
    height: auto;
    bottom: 0;
  }
  .chapter-copy {
    gap: 4px;
  }
  .chapter strong {
    font-size: 14px;
  }
  .chapter-copy > span {
    font-size: 12px;
  }
  .demo-note {
    display: block;
    font-size: 10px;
  }
  .demo-note span {
    display: block;
  }
  .workflow {
    padding-top: 64px;
    padding-bottom: 48px;
  }
  .section-heading {
    display: block;
    margin-bottom: 28px;
  }
  .section-heading p {
    margin-top: 20px;
    font-size: 14px;
  }
  h2 {
    font-size: 30px;
  }
  .feature-grid {
    grid-template-columns: 1fr;
  }
  .feature,
  .feature:nth-child(3n + 1) {
    padding: 28px 0;
    border-right: 0;
    border-bottom: 1px solid var(--line-soft);
  }
  .feature:nth-child(3n) {
    border-bottom: 0;
  }
  .feature h3 {
    font-size: 21px;
  }
  .closing {
    display: block;
    padding-top: 36px;
    padding-bottom: 44px;
  }
  .closing h2 {
    font-size: 26px;
  }
  .closing .primary {
    margin-top: 24px;
  }
}
/* --- 阶段 4：标题与实际演示共用首屏，窄屏回到阅读顺序 --- */
.hero-stage {
  display: grid;
  grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.35fr);
  align-items: center;
  gap: clamp(28px, 4vw, 64px);
  max-width: 1560px;
  margin: 0 auto;
  padding: 72px 48px 64px;
  min-height: min(760px, calc(100svh - 64px));
}
.hero-stage .hero {
  padding: 0;
  text-align: left;
}
.hero-stage .eyebrow,
.hero-stage .actions {
  justify-content: flex-start;
}
.hero-stage h1 {
  font-size: clamp(26px, 3.6vw, 54px);
}
.hero-stage h1 br,
.hero-stage .hero-copy p br {
  display: block;
}
.hero-stage .hero-copy p {
  font-size: clamp(13px, 1.15vw, 17px);
}
.hero-stage .demo {
  min-width: 0;
}
.hero-stage .chapter {
  padding: 18px 14px;
  gap: 8px;
}
.hero-stage .chapter strong {
  font-size: 14px;
}
.hero-stage .chapter-copy > span {
  display: none;
}
.hero-stage .demo-note {
  display: block;
}
.hero-stage .demo-note span + span {
  margin-left: 6px;
}
@media (min-width: 1600px) {
  .hero-stage .chapter-copy > span {
    display: block;
  }
}
@media (min-width: 701px) and (max-width: 1100px) {
  .hero-stage {
    padding: 56px 28px;
    gap: 24px;
    grid-template-columns: minmax(0, 0.85fr) minmax(0, 1.35fr);
    min-height: 620px;
  }
  .hero-stage .eyebrow {
    font-size: 10px;
    letter-spacing: 0;
  }
  .hero-stage .actions {
    gap: 8px;
    flex-wrap: wrap;
  }
  .hero-stage .actions > a {
    padding: 9px 12px;
    gap: 12px;
    font-size: 12px;
    min-height: 40px;
  }
  .hero-stage .meta {
    font-size: 10px;
    line-height: 1.8;
  }
  .hero-stage .demo-top {
    font-size: 10px;
  }
  .hero-stage .chapter {
    padding: 13px 8px;
    gap: 5px;
  }
  .hero-stage .chapter strong {
    font-size: 11px;
    line-height: 1.6;
  }
  .hero-stage .number {
    font-size: 10px;
  }
  .hero-stage .demo-note {
    font-size: 10px;
  }
}
@media (max-width: 700px) {
  .hero-stage {
    display: block;
    min-height: 0;
    padding: 40px 20px 0;
  }
  .hero-stage .hero {
    text-align: center;
    padding-bottom: 36px;
  }
  .hero-stage .eyebrow,
  .hero-stage .actions {
    justify-content: center;
  }
  .hero-stage h1 {
    font-size: clamp(26px, 7.5vw, 40px);
  }
  .hero-stage .hero-copy p {
    font-size: 14px;
  }
  .hero-stage .chapter-copy > span {
    display: block;
  }
  .hero-stage .demo-note span + span {
    margin-left: 0;
  }
}
@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
    scroll-behavior: auto !important;
  }
}
/* 与顶部导航共用宽度、断点和左右留白。 */
.hero-stage,
.wrap {
  box-sizing: border-box;
  max-width: var(--harbor-page-width);
  padding-inline: var(--harbor-page-gutter);
}
</style>
