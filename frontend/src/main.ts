import '../index.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { currentMonitor, getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalPosition } from '@tauri-apps/api/dpi';
import { isSectionId, renderSidebar, type EvidenceItem, type FlightProgress, type SectionId, type SidebarMessage, type SidebarStatus, type SystemGraphSnapshot } from './sidebar';
type UiWidget =
  | { type: 'metricCard'; label: string; value: string; unit: string; status: string }
  | { type: 'statusList'; title: string; items: string[] }
  | { type: 'notice'; title: string; body: string };

// Mirrors src/surface/schema.rs (surface/v1), camelCase + type tags.
type DockEdge = 'left' | 'right' | 'top' | 'bottom';
type WidthClass = 'narrow' | 'medium' | 'wide';
type LayoutMode = 'grid' | 'stack' | 'row';
type RegionPriority = 'primary' | 'secondary' | 'tertiary';
type Widget =
  | { type: 'metricCard'; id: string; title: string; value: string; unit: string | null; status: string | null; evidence: string[] }
  | { type: 'sensorGauge'; id: string; title: string; value: number; min: number | null; max: number | null; unit: string | null; evidence: string[] }
  | { type: 'statusList'; id: string; title: string; items: { label: string; status: string; detail: string | null }[]; evidence: string[] }
  | { type: 'chart'; id: string; title: string; data: { label: string; value: number }[]; evidence: string[] }
  | { type: 'notice'; id: string; title: string; body: string; evidence: string[] };
type Surface = {
  intent: string;
  title: string;
  subtitle: string | null;
  placement: { edge: DockEdge | null; width: WidthClass | null; float: boolean };
  layout: { mode: LayoutMode; columns: number };
  regions: { id: string; span: number; priority: RegionPriority; widgets: string[] }[];
  widgets: Widget[];
};
type PromptResponse = {
  answer: string;
  evidence: EvidenceItem[];
  widgets: UiWidget[];
  surface: Surface | null;
  experimentalHtml: string | null;
};
type BackendStatus = { ready: boolean; error: string | null };
type GraphActivityEvent = {
  phase: 'idle' | 'planning' | 'verifying' | 'gathering' | 'composing' | 'policycheck';
  activeNodeIds: string[];
  timestampMs: number;
};

const currentWindow = getCurrentWindow();
const isCanvasWindow = currentWindow.label === 'canvas';
if (isCanvasWindow) document.documentElement.classList.add('canvas-document');
let widgets: UiWidget[] = [];
let surface: Surface | null = null;
let experimentalHtml: string | null = null;
let sidebarStatus: SidebarStatus | null = null;
let sidebarStatusError: string | null = null;
let sidebarStatusRetry: number | null = null;
let graphSnapshot: SystemGraphSnapshot | null = null;
let graphSnapshotError: string | null = null;
let activeSection: SectionId = 'chat';
let requestInFlight = false;
let flightProgress: FlightProgress | null = null;
let lastSurfacePresent = false;
let chatScrollMode: 'restore' | 'end' = 'restore';
const surfacePosition = { x: 20, y: 16 };
let surfaceResizeObserver: ResizeObserver | null = null;
let inputRegionFrame: number | null = null;
let dragState: {
  pointerId: number;
  startX: number;
  startY: number;
  left: number;
  top: number;
  handle: HTMLElement;
} | null = null;
const messages: SidebarMessage[] = [{
  role: 'assistant',
  text: 'I’m ready to investigate your system. Ask me what you would like to know.',
  state: 'complete',
}];

type SidebarDomSnap = {
  value: string;
  start: number;
  end: number;
  height: string;
  chatScroll: number;
  promptFocused: boolean;
  railSection: string | null;
};

function snapshotSidebarDom(): SidebarDomSnap {
  const prompt = document.querySelector<HTMLTextAreaElement>('#prompt');
  const chat = document.querySelector<HTMLElement>('.chat');
  const active = document.activeElement as HTMLElement | null;
  return {
    value: prompt?.value ?? '',
    start: prompt?.selectionStart ?? 0,
    end: prompt?.selectionEnd ?? 0,
    height: prompt?.style.height ?? '',
    chatScroll: chat?.scrollTop ?? 0,
    promptFocused: active?.id === 'prompt',
    railSection: active?.dataset.section ?? null,
  };
}

function restoreSidebarDom(snap: SidebarDomSnap): void {
  const prompt = document.querySelector<HTMLTextAreaElement>('#prompt');
  const chat = document.querySelector<HTMLElement>('.chat');
  if (prompt) {
    prompt.value = snap.value;
    prompt.style.height = snap.height;
    autosizePrompt(prompt);
    updatePromptSend(prompt);
    if (snap.promptFocused) {
      prompt.focus();
      prompt.setSelectionRange(snap.start, snap.end);
    }
  }
  if (snap.railSection) {
    document.querySelector<HTMLButtonElement>(`.rail-btn[data-section="${snap.railSection}"]`)?.focus();
  }
  if (chat) {
    chat.scrollTop = chatScrollMode === 'end' ? chat.scrollHeight : snap.chatScroll;
  }
  chatScrollMode = 'restore';
}

function bindSidebar(): void {
  document.querySelector<HTMLFormElement>('#prompt-form')?.addEventListener('submit', submitPrompt);
  const promptEl = document.querySelector<HTMLTextAreaElement>('#prompt');
  promptEl?.addEventListener('pointerdown', () => {
    void invoke('focus_sidebar');
  });
  promptEl?.addEventListener('input', () => {
    autosizePrompt(promptEl);
    updatePromptSend(promptEl);
  });
  promptEl?.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      promptEl.form?.requestSubmit();
    }
  });
  document.querySelectorAll<HTMLButtonElement>('.rail-btn').forEach((button) => {
    button.addEventListener('click', () => {
      const section = button.dataset.section;
      if (!isSectionId(section)) return;
      activeSection = activeSection === section && section !== 'chat' ? 'chat' : section;
      render();
    });
  });
  document.querySelector<HTMLButtonElement>('[data-dismiss-inspector]')?.addEventListener('click', () => {
    activeSection = 'chat';
    render();
  });
  document.querySelectorAll<HTMLButtonElement>('[data-retry]').forEach((button) => {
    button.addEventListener('click', () => {
      void retryLastRequest();
    });
  });
}

function render(): void {
  const root = document.querySelector<HTMLDivElement>('#root');
  if (!root) return;
  const snap = isCanvasWindow ? null : snapshotSidebarDom();
  root.innerHTML = isCanvasWindow
    ? renderCanvas()
    : renderSidebar({
        messages,
        status: sidebarStatus,
        statusError: sidebarStatusError,
        section: activeSection,
        requestInFlight,
        flightProgress,
        hasSurface: lastSurfacePresent,
        graph: graphSnapshot,
        graphError: graphSnapshotError,
      }, escapeHtml);
  if (!isCanvasWindow) {
    bindSidebar();
    if (snap) restoreSidebarDom(snap);
  } else {
    document.querySelectorAll<HTMLButtonElement>('[data-dock]').forEach((button) => {
      button.addEventListener('click', () => void dockPanel(button.dataset.dock as DockEdge));
    });
    if (experimentalHtml) {
      wireSurfaceDrag();
      observeSurfaceSize();
    } else {
      surfaceResizeObserver?.disconnect();
      surfaceResizeObserver = null;
    }
  }
}

async function refreshSidebarStatus(): Promise<void> {
  try {
    sidebarStatus = await invoke<SidebarStatus>('sidebar_status');
    sidebarStatusError = null;
    if (sidebarStatusRetry !== null) {
      window.clearTimeout(sidebarStatusRetry);
      sidebarStatusRetry = null;
    }
  } catch (error) {
    try {
      const backend = await invoke<BackendStatus>('backend_status');
      sidebarStatusError = backend.error ?? String(error);
      if (!backend.ready && backend.error === 'backend is starting' && sidebarStatusRetry === null) {
        sidebarStatusRetry = window.setTimeout(() => {
          sidebarStatusRetry = null;
          void refreshSidebarStatus();
        }, 500);
      }
    } catch (statusError) {
      sidebarStatusError = `${String(error)}; readiness check failed: ${String(statusError)}`;
    }
  }
  if (!isCanvasWindow) render();
}

async function refreshGraph(): Promise<void> {
  try {
    graphSnapshot = await invoke<SystemGraphSnapshot>('system_graph');
    graphSnapshotError = null;
  } catch (error) {
    graphSnapshotError = String(error);
  }
  if (!isCanvasWindow) render();
}

function renderCanvas(): string {
  if (experimentalHtml) {
    return `<div id="surface-host" class="surface-host" style="left:${surfacePosition.x}px;top:${surfacePosition.y}px">${experimentalHtml}</div>`;
  }
  const heading = surface
    ? `<div class="eyebrow">Generative surface</div><h1>${escapeHtml(surface.title)}</h1>${surface.subtitle ? `<div class="canvas-subtitle">${escapeHtml(surface.subtitle)}</div>` : ''}`
    : `<div class="eyebrow">Live evidence</div><h1>System overview</h1>`;
  const body = surface
    ? renderSurface(surface)
    : `<div class="widget-grid">${widgets.map(renderWidgetLegacy).join('')}</div>`;
  return `<main class="canvas-window"><header class="canvas-header"><div>${heading}</div><div><div class="connection-state"><span class="status-dot"></span> Verified backend evidence</div><nav class="dock-actions" aria-label="Dock panel"><button data-dock="left">Left</button><button data-dock="right">Right</button><button data-dock="top">Top</button><button data-dock="bottom">Bottom</button></nav></div></header>${body}</main>`;
}

async function dockPanel(edge: DockEdge): Promise<void> {
  const monitor = await currentMonitor();
  if (!monitor) return;
  const size = await currentWindow.outerSize();
  const workArea = monitor.workArea;
  let x = workArea.position.x;
  let y = workArea.position.y;
  if (edge === 'right') x += workArea.size.width - size.width;
  if (edge === 'bottom') y += workArea.size.height - size.height;
  if (edge === 'top' || edge === 'bottom') x += Math.max(0, (workArea.size.width - size.width) / 2);
  if (edge === 'left' || edge === 'right') y += Math.max(0, (workArea.size.height - size.height) / 2);
  await currentWindow.setPosition(new PhysicalPosition(x, y));
}

function autosizePrompt(el: HTMLTextAreaElement): void {
  el.style.height = 'auto';
  el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
}

function updatePromptSend(el: HTMLTextAreaElement): void {
  const send = document.querySelector<HTMLButtonElement>('.prompt-send');
  if (send) send.disabled = requestInFlight || !el.value.trim();
}

async function submitPrompt(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const input = document.querySelector<HTMLTextAreaElement>('#prompt');
  const text = input?.value.trim();
  if (!input || !text || requestInFlight) return;
  input.value = '';
  autosizePrompt(input);
  updatePromptSend(input);
  await runPrompt(text, true);
}

async function retryLastRequest(): Promise<void> {
  if (requestInFlight) return;
  const lastUser = [...messages].reverse().find((message) => message.role === 'user');
  if (!lastUser) return;
  const last = messages[messages.length - 1];
  if (last?.state === 'failed') messages.pop();
  await runPrompt(lastUser.text, false);
}

function visualNodeId(id: string): string {
  if (id.startsWith('specialist:')) return id.split(':')[1] ?? id;
  if (id.startsWith('guardian:')) return 'guardian';
  return id;
}

function activityLabel(phase: GraphActivityEvent['phase']): string {
  return phase === 'policycheck' ? 'Policy check' : phase ? phase[0].toUpperCase() + phase.slice(1) : 'Idle';
}

function activityDetail(phase: GraphActivityEvent['phase'], activeNodeIds: string[]): string {
  const nodes = activeNodeIds.length ? ` · ${activeNodeIds.join(', ')}` : '';
  switch (phase) {
    case 'planning': return `Planner evaluating system requirements and tools${nodes}`;
    case 'verifying': return `Verifier reviewing the plan${nodes}`;
    case 'gathering': return `Broker gathering specialist evidence${nodes}`;
    case 'composing': return `SurfaceComposer generating the presentation${nodes}`;
    case 'policycheck': return `Broker consulting Guardian${nodes}`;
    default: return 'Backend idle';
  }
}

function applyGraphActivity(event: GraphActivityEvent): void {
  const activeNodeIds = event.activeNodeIds.map(visualNodeId);
  const phase = event.phase;
  if (graphSnapshot) {
    graphSnapshot = {
      ...graphSnapshot,
      phase,
      activeNodeIds,
      nodes: graphSnapshot.nodes.map((node) => ({
        ...node,
        active: activeNodeIds.includes(node.id),
      })),
    };
  }
  flightProgress = phase === 'idle'
    ? null
    : {
        phase,
        specialists: activeNodeIds.filter((id) => ['wifi', 'storage', 'network', 'drivers', 'graphics', 'memory', 'power', 'processes', 'security', 'boot', 'packages'].includes(id)),
        label: activityLabel(phase),
        detail: activityDetail(phase, activeNodeIds),
        activeNodeIds,
      };
  render();
}

async function runPrompt(text: string, pushUser: boolean): Promise<void> {
  if (requestInFlight) return;
  flightProgress = null;
  requestInFlight = true;
  if (pushUser) messages.push({ role: 'user', text, state: 'complete' });
  chatScrollMode = 'end';
  render();

  try {
    const response = await invoke<PromptResponse>('submit_prompt', { prompt: text });
    const next: SidebarMessage = {
      role: 'assistant',
      text: response.answer,
      evidence: response.evidence,
      state: 'complete',
    };
    messages.push(next);
    surface = response.surface;
    widgets = response.widgets;
    lastSurfacePresent = Boolean(response.surface || response.experimentalHtml);
    void refreshSidebarStatus();
    void refreshGraph();
  } catch (error) {
    messages.push({
      role: 'assistant',
      text: `Backend unavailable: ${String(error)}`,
      state: 'failed',
    });
  } finally {
    flightProgress = null;
    requestInFlight = false;
    chatScrollMode = 'end';
    render();
  }
}

// --- Surface rendering (surface/v1) ---

const PRIORITY_ORDER: Record<RegionPriority, number> = { primary: 0, secondary: 1, tertiary: 2 };

function renderSurface(s: Surface): string {
  const widgetById = new Map(s.widgets.map((widget) => [widget.id, widget]));
  const regions = [...s.regions].sort(
    (a, b) => (PRIORITY_ORDER[a.priority] ?? 3) - (PRIORITY_ORDER[b.priority] ?? 3),
  );
  const modeClass = s.layout.mode === 'stack' ? 'surface-stack' : s.layout.mode === 'row' ? 'surface-row' : 'surface-grid';
  const columns = Math.max(1, s.layout.columns || 12);
  const gridStyle = s.layout.mode === 'grid' ? `style="grid-template-columns:repeat(${columns},minmax(0,1fr))"` : '';
  return `<div class="surface ${modeClass}" ${gridStyle}>${regions
    .map((region) => {
      const regionWidgets = region.widgets
        .map((id) => widgetById.get(id))
        .filter((widget): widget is Widget => Boolean(widget))
        .map(renderWidget);
      const span = s.layout.mode === 'grid' ? ` style="grid-column:span ${Math.max(1, Math.min(region.span, columns))}"` : '';
      return `<section class="surface-region region-${escapeHtml(region.priority)}"${span}><div class="region-label">${escapeHtml(region.id)}</div>${regionWidgets.join('')}</section>`;
    })
    .join('')}</div>`;
}

function renderWidget(widget: Widget): string {
  switch (widget.type) {
    case 'metricCard':
      return `<article class="surface-widget metric-widget"><div class="widget-label">${escapeHtml(widget.title)}</div><div class="metric-value">${escapeHtml(widget.value)} ${widget.unit ? `<small>${escapeHtml(widget.unit)}</small>` : ''}</div>${widget.status ? `<div class="widget-state">${escapeHtml(widget.status)}</div>` : ''}${evidenceChips(widget.evidence)}</article>`;
    case 'sensorGauge':
      return `<article class="surface-widget gauge-widget"><div class="widget-label">${escapeHtml(widget.title)}</div><div class="gauge-row"><div class="gauge-track"><div class="gauge-fill" style="width:${gaugePercent(widget)}%"></div></div><div class="gauge-value">${formatNumber(widget.value)}${widget.unit ? `<small> ${escapeHtml(widget.unit)}</small>` : ''}</div></div>${evidenceChips(widget.evidence)}</article>`;
    case 'statusList':
      return `<article class="surface-widget status-widget"><h2>${escapeHtml(widget.title)}</h2><ul>${widget.items.map((item) => `<li><span class="status-item-label">${escapeHtml(item.label)}</span><span class="status-item-value">${escapeHtml(item.status)}</span>${item.detail ? `<small class="status-item-detail">${escapeHtml(item.detail)}</small>` : ''}</li>`).join('')}</ul>${evidenceChips(widget.evidence)}</article>`;
    case 'chart':
      return `<article class="surface-widget chart-widget"><h2>${escapeHtml(widget.title)}</h2><div class="chart-bars">${renderChartBars(widget)}</div>${evidenceChips(widget.evidence)}</article>`;
    case 'notice':
      return `<article class="surface-widget notice-widget"><h2>${escapeHtml(widget.title)}</h2><p>${escapeHtml(widget.body)}</p>${evidenceChips(widget.evidence)}</article>`;
  }
}

function gaugePercent(widget: Widget & { type: 'sensorGauge' }): number {
  const min = widget.min ?? 0;
  const max = widget.max ?? 100;
  const span = max - min;
  if (span <= 0) return 0;
  const pct = ((widget.value - min) / span) * 100;
  return Math.max(0, Math.min(100, pct));
}

function renderChartBars(widget: Widget & { type: 'chart' }): string {
  const max = Math.max(1, ...widget.data.map((point) => Math.abs(point.value)));
  return widget.data
    .map((point) => {
      const height = Math.max(2, (Math.abs(point.value) / max) * 100);
      return `<div class="chart-col"><div class="chart-bar" style="height:${height}%"></div><div class="chart-label">${escapeHtml(point.label)}</div><div class="chart-value">${formatNumber(point.value)}</div></div>`;
    })
    .join('');
}

function evidenceChips(keys: string[]): string {
  if (!keys.length) return '';
  return `<div class="evidence-chips">${keys.map((key) => `<span class="evidence-chip">${escapeHtml(key)}</span>`).join('')}</div>`;
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(value);
}

function renderWidgetLegacy(widget: UiWidget): string {
  switch (widget.type) {
    case 'metricCard':
      return `<article class="widget metric-widget"><div class="widget-label">${escapeHtml(widget.label)}</div><div class="metric-value">${escapeHtml(widget.value)} <small>${escapeHtml(widget.unit)}</small></div><div class="widget-state">${escapeHtml(widget.status)}</div></article>`;
    case 'statusList':
      return `<article class="widget"><h2>${escapeHtml(widget.title)}</h2><ul>${widget.items.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul></article>`;
    case 'notice':
      return `<article class="widget notice-widget"><h2>${escapeHtml(widget.title)}</h2><p>${escapeHtml(widget.body)}</p></article>`;
  }
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' })[character] ?? character);
}

function surfaceHost(): HTMLElement | null {
  return document.querySelector<HTMLElement>('#surface-host');
}

function scheduleInputRegion(): void {
  if (inputRegionFrame !== null) return;
  inputRegionFrame = requestAnimationFrame(() => {
    inputRegionFrame = null;
    void updateInputRegion();
  });
}

async function updateInputRegion(): Promise<void> {
  const target = experimentalHtml ? surfaceHost() : document.querySelector<HTMLElement>('#root');
  if (!target) {
    await invoke('set_input_region', { x: 0, y: 0, w: 0, h: 0 });
    return;
  }
  const rect = target.getBoundingClientRect();
  const scale = window.devicePixelRatio || 1;
  await invoke('set_input_region', {
    x: rect.left * scale,
    y: rect.top * scale,
    w: rect.width * scale,
    h: rect.height * scale,
  });
}

function observeSurfaceSize(): void {
  surfaceResizeObserver?.disconnect();
  const host = surfaceHost();
  if (!host) return;
  surfaceResizeObserver = new ResizeObserver(() => scheduleInputRegion());
  surfaceResizeObserver.observe(host);
}

function wireSurfaceDrag(): void {
  const host = surfaceHost();
  if (!host) return;
  const handle = host.querySelector<HTMLElement>('[data-tauri-drag-region], header') ?? host;
  handle.addEventListener('pointerdown', (event) => {
    if (event.button !== 0) return;
    if ((event.target as Element).closest('button, a, input, textarea, select, [data-no-drag]')) return;
    const rect = host.getBoundingClientRect();
    dragState = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      left: rect.left,
      top: rect.top,
      handle,
    };
    handle.setPointerCapture?.(event.pointerId);
    handle.classList.add('surface-dragging');
    event.preventDefault();
  });
  handle.addEventListener('pointermove', (event) => {
    if (!dragState || dragState.pointerId !== event.pointerId) return;
    const root = document.querySelector<HTMLElement>('#root');
    const maxLeft = root ? Math.max(0, root.clientWidth - host.offsetWidth) : Number.POSITIVE_INFINITY;
    const maxTop = root ? Math.max(0, root.clientHeight - host.offsetHeight) : Number.POSITIVE_INFINITY;
    surfacePosition.x = Math.min(maxLeft, Math.max(0, dragState.left + event.clientX - dragState.startX));
    surfacePosition.y = Math.min(maxTop, Math.max(0, dragState.top + event.clientY - dragState.startY));
    host.style.left = `${surfacePosition.x}px`;
    host.style.top = `${surfacePosition.y}px`;
    scheduleInputRegion();
  });
  const endDrag = (event: PointerEvent) => {
    if (!dragState || dragState.pointerId !== event.pointerId) return;
    try {
      dragState.handle.releasePointerCapture?.(event.pointerId);
    } catch {
      // The pointer may already have left the webview during a desktop drag.
    }
    dragState.handle.classList.remove('surface-dragging');
    dragState = null;
    scheduleInputRegion();
  };
  handle.addEventListener('pointerup', endDrag);
  handle.addEventListener('pointercancel', endDrag);
}

if (isCanvasWindow) {
  void listen<PromptResponse>('canvas_response', async (event) => {
    experimentalHtml = event.payload.experimentalHtml ?? null;
    surface = event.payload.surface;
    widgets = event.payload.widgets;
    render();
    if (experimentalHtml) {
      // Wait for WebKitGTK to finish layout, then expose only the widget area
      // to the desktop input system.
      await new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));
    }
    await updateInputRegion();
    await currentWindow.show();
  }).catch((error) => {
    console.error(`[Aios] canvas event listener failed: ${String(error)}`);
  });
}

if (!isCanvasWindow) {
  void listen<GraphActivityEvent>('graph_activity', (event) => {
    applyGraphActivity(event.payload);
  }).catch((error) => {
    graphSnapshotError = `Activity stream unavailable: ${String(error)}`;
    render();
  });
  void refreshSidebarStatus();
  void refreshGraph();
  window.setInterval(() => {
    if (!requestInFlight) void refreshGraph();
  }, 8000);
}

render();
