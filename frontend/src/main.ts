import '../index.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { currentMonitor, getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalPosition } from '@tauri-apps/api/dpi';
import { isSectionId, providerCatalog, renderSidebar, roleState, rolesCatalog, settingsForm, updateProviderCatalog, updateRolesCatalog, updateSettingsProviders, type EvidenceItem, type FlightProgress, type SectionId, type SidebarMessage, type SidebarStatus, type SystemGraphSnapshot } from './sidebar';
// The only dock edge type still shared with the sidebar renderer.
type DockEdge = 'left' | 'right' | 'top' | 'bottom';
type PromptResponse = {
  answer: string;
  evidence: EvidenceItem[];
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
      if (activeSection === 'settings') void syncRoleAssignmentsFromBackend();
      render();
    });
  });
  document.querySelectorAll<HTMLButtonElement>('[data-dismiss-inspector]')?.forEach((button) => {
    button.addEventListener('click', () => {
      activeSection = 'chat';
      settingsForm.error = null;
      render();
    });
  });
  document.querySelectorAll<HTMLButtonElement>('[data-remove-provider]').forEach((button) => {
    button.addEventListener('click', () => {
      void removeProvider(button.dataset.removeProvider ?? '');
    });
  });
  document.querySelectorAll<HTMLButtonElement>('[data-set-key]').forEach((button) => {
    button.addEventListener('click', () => {
      void setProviderKey(button.dataset.setKey ?? '');
    });
  });
  const providerForm = document.querySelector<HTMLFormElement>('#provider-form');
  providerForm?.addEventListener('submit', (event) => {
    event.preventDefault();
    void addProvider(new FormData(providerForm));
  });
  document.querySelectorAll<HTMLFormElement>('.settings-role').forEach((form) => {
    form.addEventListener('submit', (event) => {
      event.preventDefault();
      void assignRole(form.dataset.role ?? '', new FormData(form));
    });
  });
  const bulkForm = document.querySelector<HTMLFormElement>('#bulk-form');
  bulkForm?.addEventListener('submit', (event) => {
    event.preventDefault();
    void assignRoleGroup(new FormData(bulkForm));
  });
  document.querySelectorAll<HTMLButtonElement>('[data-retry]').forEach((button) => {
    button.addEventListener('click', () => {
      void retryLastRequest();
    });
  });
  bindSelectCloser();
}

function closeAllSelects(): void {
  document.querySelectorAll<HTMLElement>('.aios-select').forEach((box) => {
    box.classList.remove('open');
    box.querySelector<HTMLElement>('.aios-select-list')?.setAttribute('hidden', '');
    box.querySelector<HTMLButtonElement>('.aios-select-trigger')?.setAttribute('aria-expanded', 'false');
  });
}

let selectCloserBound = false;

/// One delegated listener drives every custom dropdown. It uses pointerdown
/// (capture), not click: this frameless WebKitGTK window can drop or reroute
/// click events (the focus_sidebar hack exists for the same reason), and
/// per-node click listeners went stale when a pick did not trigger a render,
/// leaving the listbox open.
function bindSelectCloser(): void {
  if (selectCloserBound) return;
  selectCloserBound = true;
  document.addEventListener('pointerdown', (event) => {
    const target = event.target as Element;
    const item = target.closest?.('.aios-select-list li');
    if (item) {
      const box = item.closest<HTMLElement>('.aios-select');
      if (box) pickSelectOption(box, item.dataset.value ?? '', item.textContent ?? '');
      return;
    }
    // Scrollbar/drag inside an open list: keep it open.
    if (target.closest?.('.aios-select-list')) return;
    const trigger = target.closest?.('.aios-select-trigger');
    if (trigger) {
      const box = trigger.closest<HTMLElement>('.aios-select');
      const list = box?.querySelector<HTMLElement>('.aios-select-list');
      if (!box || !list) return;
      const wasOpen = !list.hidden;
      closeAllSelects();
      if (!wasOpen) {
        list.hidden = false;
        box.classList.add('open');
        trigger.setAttribute('aria-expanded', 'true');
      }
      return;
    }
    closeAllSelects();
  }, true);
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') closeAllSelects();
  });
}

function pickSelectOption(box: HTMLElement, value: string, label: string): void {
  // Close this box first so a missing re-render can never leave it open.
  box.classList.remove('open');
  box.querySelector<HTMLElement>('.aios-select-list')?.setAttribute('hidden', '');
  box.querySelector<HTMLButtonElement>('.aios-select-trigger')?.setAttribute('aria-expanded', 'false');
  const input = box.querySelector<HTMLInputElement>('input[type="hidden"]');
  if (input) input.value = value;
  const valueEl = box.querySelector<HTMLElement>('.aios-select-value');
  if (valueEl) valueEl.textContent = label;
  box.classList.toggle('is-placeholder', !value);
  box.querySelectorAll<HTMLLIElement>('.aios-select-list li').forEach((li) => {
    li.setAttribute('aria-selected', String(li.dataset.value === value));
    li.classList.toggle('is-selected', li.dataset.value === value);
  });
  closeAllSelects();
  const role = box.dataset.roleProvider;
  if (role && value) void loadModelsForRole(role, value);
  if (box.hasAttribute('data-bulk-provider') && value) void loadModelsForBulk(value);
  if (box.hasAttribute('data-catalog-select')) syncCatalogSelection(value);
}

/// Endpoint auto-fill for the Add provider form.
function syncCatalogSelection(catalogId: string): void {
  settingsForm.catalogId = catalogId;
  settingsForm.providerEndpoint = providerCatalog.find((c) => c.id === catalogId)?.endpoint ?? '';
  const endpointInput = document.querySelector<HTMLInputElement>('#provider-form input[name="endpoint"]');
  if (endpointInput) endpointInput.value = settingsForm.providerEndpoint;
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
    updateSettingsProviders(sidebarStatus.providers);
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

// ---- Settings panel actions ----

async function addProvider(_form: FormData): Promise<void> {
  const catalogId = String(_form.get('catalog_id') ?? '').trim();
  const endpoint = String(_form.get('endpoint') ?? '').trim();
  const apiKey = String(_form.get('api_key') ?? '').trim();
  if (!catalogId || !endpoint || !apiKey) {
    settingsForm.error = 'Provider, endpoint, and API key are all required.';
    render();
    return;
  }
  const catalog = providerCatalog.find((c) => c.id === catalogId);
  if (!catalog) {
    settingsForm.error = `Unknown provider '${catalogId}'.`;
    render();
    return;
  }
  settingsForm.busy = true;
  settingsForm.error = null;
  render();
  try {
    await invoke('add_provider', {
      id: catalogId,
      kind: catalog.kind,
      tier: catalog.tier,
      endpoint,
      model: null,
      apiKey,
      httpTimeoutMs: 60000,
    });
    settingsForm.catalogId = '';
    settingsForm.providerEndpoint = '';
    settingsForm.providerKey = '';
    await refreshSidebarStatus();
  } catch (error) {
    settingsForm.error = String(error);
  } finally {
    settingsForm.busy = false;
    render();
  }
}

async function loadModelsForRole(role: string, providerId: string): Promise<void> {
  const state = roleState(role);
  settingsForm.busy = true;
  settingsForm.error = null;
  state.provider = providerId;
  state.model = '';
  render();
  try {
    const models = await invoke<{ id: string; name: string | null }[]>('discover_models', { providerId });
    state.models = models;
    state.discoveryError = models.length
      ? null
      : `no models were found for provider '${providerId}'`;
  } catch (error) {
    state.models = [];
    state.discoveryError = String(error);
  } finally {
    settingsForm.busy = false;
    render();
  }
}

async function loadModelsForBulk(providerId: string): Promise<void> {
  settingsForm.busy = true;
  settingsForm.error = null;
  settingsForm.bulkProvider = providerId;
  settingsForm.bulkModel = '';
  render();
  try {
    const models = await invoke<{ id: string; name: string | null }[]>('discover_models', { providerId });
    settingsForm.bulkModels = models;
    settingsForm.bulkDiscoveryError = models.length
      ? null
      : `no models were found for provider '${providerId}'`;
  } catch (error) {
    settingsForm.bulkModels = [];
    settingsForm.bulkDiscoveryError = String(error);
  } finally {
    settingsForm.busy = false;
    render();
  }
}

async function assignRoleGroup(form: FormData): Promise<void> {
  const group = String(form.get('group') ?? '').trim();
  const providerId = String(form.get('provider') ?? '').trim();
  const model = String(form.get('model') ?? '').trim();
  if (!group || !providerId || !model) return;
  settingsForm.busy = true;
  settingsForm.error = null;
  try {
    const assigned = await invoke<string[]>('set_role_group_assignment', { group, providerId, model });
    // Reflect the group assignment on every affected role row, reusing the
    // model list already discovered for the bulk form.
    for (const role of assigned) {
      const state = roleState(role);
      state.provider = providerId;
      state.model = model;
      state.models = settingsForm.bulkModels;
      state.discoveryError = settingsForm.bulkDiscoveryError;
    }
    settingsForm.bulkModel = model;
    await refreshSidebarStatus();
  } catch (error) {
    settingsForm.error = String(error);
  } finally {
    settingsForm.busy = false;
    render();
  }
}

async function removeProvider(id: string): Promise<void> {
  if (!id) return;
  settingsForm.busy = true;
  settingsForm.error = null;
  try {
    await invoke('remove_provider', { id });
    // The backend dropped assignments referencing this provider; reset the
    // panel rows that pointed at it.
    for (const state of Object.values(settingsForm.roles)) {
      if (state.provider === id) {
        state.provider = '';
        state.model = '';
        state.models = [];
        state.discoveryError = null;
      }
    }
    await refreshSidebarStatus();
  } catch (error) {
    settingsForm.error = String(error);
  } finally {
    settingsForm.busy = false;
    render();
  }
}

async function setProviderKey(id: string): Promise<void> {
  const key = window.prompt(`API key for ${id}`);
  if (!key) return;
  settingsForm.busy = true;
  settingsForm.error = null;
  try {
    await invoke('set_provider_credential', { id, apiKey: key });
    await refreshSidebarStatus();
  } catch (error) {
    settingsForm.error = String(error);
  } finally {
    settingsForm.busy = false;
    render();
  }
}

async function assignRole(role: string, form: FormData): Promise<void> {
  const providerId = String(form.get('provider') ?? '').trim();
  const model = String(form.get('model') ?? '').trim();
  if (!providerId || !model) return;
  settingsForm.busy = true;
  settingsForm.error = null;
  try {
    await invoke('set_role_assignment', { role, providerId, model });
    roleState(role).model = model;
    await refreshSidebarStatus();
  } catch (error) {
    settingsForm.error = String(error);
  } finally {
    settingsForm.busy = false;
    render();
  }
}

async function loadProviderCatalog(): Promise<void> {
  try {
    const catalog = await invoke<{ id: string; label: string; endpoint: string; kind: string; tier: string }[]>('provider_catalog');
    updateProviderCatalog(catalog);
  } catch (error) {
    graphSnapshotError = `Provider catalog unavailable: ${String(error)}`;
  }
  try {
    const roles = await invoke<{ id: string; label: string; detail: string; fit: string }[]>('roles_catalog');
    updateRolesCatalog(roles);
  } catch (error) {
    graphSnapshotError = `Roles catalog unavailable: ${String(error)}`;
  }
  if (!isCanvasWindow) render();
  void syncRoleAssignmentsFromBackend();
}

let roleSyncInFlight = false;

/// Pull the current assignment for every assignable role from the backend so
/// the settings rows show persisted state instead of placeholders (fresh
/// session, edits made elsewhere, etc.). Local edits keep updating state
/// directly; this only fills what the backend already knows.
async function syncRoleAssignmentsFromBackend(): Promise<void> {
  if (roleSyncInFlight || isCanvasWindow) return;
  roleSyncInFlight = true;
  try {
    for (const role of rolesCatalog) {
      const route = await invoke<{ provider: string; model: string } | null>('role_route', { role: role.id });
      if (!route) continue;
      const state = roleState(role.id);
      if (state.provider !== route.provider || state.model !== route.model) {
        state.provider = route.provider;
        state.model = route.model;
      }
    }
    if (activeSection === 'settings') render();
  } catch {
    // Status stays as-is; the rows fall back to manual discovery.
  } finally {
    roleSyncInFlight = false;
  }
}

async function refreshGraph(): Promise<void> {
  try {
    graphSnapshot = await invoke<SystemGraphSnapshot>('system_graph');
    graphSnapshotError = null;
  } catch (error) {
    graphSnapshotError = String(error);
  }
  // While the settings panel is open, refresh data silently. Rebuilding the
  // DOM mid-interaction destroys the provider dropdown popup, clears the
  // selection a moment later, and wipes anything typed into the form.
  if (!isCanvasWindow && activeSection !== 'settings') render();
}

function renderCanvas(): string {
  if (!experimentalHtml) return '';
  return `<div id="surface-host" class="surface-host" style="left:${surfacePosition.x}px;top:${surfacePosition.y}px">${experimentalHtml}</div>`;
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
  // Never rebuild the DOM under an open settings form: a rebuild between
  // mousedown and focus makes text fields unclickable and wipes typed input.
  if (!isCanvasWindow && activeSection !== 'settings') render();
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
    experimentalHtml = response.experimentalHtml;
    lastSurfacePresent = Boolean(response.experimentalHtml);
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

// --- Generated surface host ---
// The HTML comes verbatim from the groundless surface model (already passed
// the fidelity gate in the backend). The frontend only hosts, drags, and
// measures it.

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
  // The frameless sidebar window can hold DOM focus without holding real X11
  // keyboard focus; keystrokes then vanish. The prompt re-establishes X11
  // focus on pointerdown via focus_sidebar. Extend the same fix to every
  // form control, including the settings modal fields.
  document.addEventListener('pointerdown', (event) => {
    const target = event.target as Element;
    if (target.closest?.('input, textarea, select, button')) {
      void invoke('focus_sidebar').catch(() => {});
    }
  }, true);
  void listen<GraphActivityEvent>('graph_activity', (event) => {
    applyGraphActivity(event.payload);
  }).catch((error) => {
    graphSnapshotError = `Activity stream unavailable: ${String(error)}`;
    render();
  });
  void refreshSidebarStatus();
  void refreshGraph();
  void loadProviderCatalog();
  window.setInterval(() => {
    if (!requestInFlight) void refreshGraph();
  }, 8000);
}

render();
