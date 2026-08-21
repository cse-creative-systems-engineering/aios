export type EvidenceItem = { tool: string; text: string };
export type MessageState = 'pending' | 'complete' | 'failed';
export type SidebarMessage = {
  role: 'user' | 'assistant';
  text: string;
  evidence?: EvidenceItem[];
  state?: MessageState;
};

export type SidebarRoute = {
  provider: string;
  model: string;
  connectivity: string;
  dataClassification: string;
  reducedConfidence: boolean;
};

export type SidebarStatus = {
  backendStatus: { ready: boolean; error: string | null };
  connectivity: string;
  currentRoute: SidebarRoute | null;
  chatRoute: SidebarRoute | null;
  routeError: string | null;
  localModel: string | null;
  providers: {
    id: string;
    kind: string;
    model: string;
    tier: string;
    capabilities: string[];
    health: string;
    lastChecked: number;
    latencyMs: number | null;
    errorRate: number;
    retryAfter: number | null;
    credentialConfigured: boolean;
    consentScopes: string[];
  }[];
};

export type SectionId = 'chat' | 'providers' | 'roles' | 'surfaces' | 'audit' | 'settings';

export type GraphNode = {
  id: string;
  label: string;
  layer: string;
  nodeType: string;
  health: string;
  active: boolean;
  detail: string;
};

export type GraphEdge = {
  from: string;
  to: string;
  edgeType: string;
};

export type SystemGraphSnapshot = {
  nodes: GraphNode[];
  edges: GraphEdge[];
  totalNodes: number;
  healthCounts: [string, number][];
  phase: string;
  activeNodeIds: string[];
};

export type FlightProgress = {
  phase: 'planning' | 'verifying' | 'gathering' | 'composing' | 'policycheck';
  specialists: string[];
  label: string;
  detail: string;
  activeNodeIds: string[];
};

export type SidebarView = {
  messages: SidebarMessage[];
  status: SidebarStatus | null;
  statusError: string | null;
  graphError: string | null;
  section: SectionId;
  requestInFlight: boolean;
  flightProgress: FlightProgress | null;
  hasSurface: boolean;
  graph: SystemGraphSnapshot | null;
};

const GRAPH_LAYER_Y: Record<string, number> = {
  orchestration: 22,
  agent: 72,
  model: 122,
  specialist: 172,
  infrastructure: 232,
  surface: 278,
};
const GRAPH_W = 420;
const GRAPH_H = 310;
const NODE_W = 56;
const NODE_H = 22;

const RAIL_ICONS: Record<SectionId, string> = {
  chat: `<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M2 13V4a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H6l-3 3z"/><path d="M5 6h8M5 9h5"/></svg>`,
  providers: `<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="14" height="4" rx="1"/><rect x="2" y="7" width="14" height="4" rx="1"/><rect x="2" y="12" width="14" height="4" rx="1"/></svg>`,
  roles: `<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="5.5" height="5.5" rx="1"/><rect x="10.5" y="2" width="5.5" height="5.5" rx="1"/><rect x="2" y="10.5" width="5.5" height="5.5" rx="1"/><rect x="10.5" y="10.5" width="5.5" height="5.5" rx="1"/></svg>`,
  surfaces: `<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M3 5l6-2.5L15 5l-6 2.5z"/><path d="M3 9l6 2.5L15 9"/><path d="M3 13l6 2.5L15 13"/></svg>`,
  audit: `<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M9 2L4 4.5v4c0 3.5 2.2 6.3 5 7.5 2.8-1.2 5-4 5-7.5v-4z"/><path d="M6.5 9l2 2 3.5-3.5"/></svg>`,
  settings: `<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="9" cy="9" r="2.5"/><path d="M9 1.5v2M9 14.5v2M1.5 9h2M14.5 9h2M3.3 3.3l1.4 1.4M13.3 13.3l1.4 1.4M3.3 14.7l1.4-1.4M13.3 4.7l1.4-1.4"/></svg>`,
};

const SECTION_LABELS: Record<SectionId, string> = {
  chat: 'Chat',
  providers: 'Providers',
  roles: 'Roles',
  surfaces: 'Surfaces',
  audit: 'Audit',
  settings: 'Settings',
};

const INSPECTOR: Record<Exclude<SectionId, 'chat'>, { title: string; body: string }> = {
  providers: {
    title: 'Providers',
    body: 'Provider administration is not connected. Credentials never enter this surface. The backend still owns the registry, health checks, and discovery. This view will list those records when the settings IPC exists.',
  },
  roles: {
    title: 'Roles',
    body: 'Role assignment is not connected. Planner, Verifier, SurfaceComposition, and specialist overrides stay on the backend registry. The Policy Broker is not a model slot and will not appear here.',
  },
  surfaces: {
    title: 'Surfaces',
    body: 'Surface lifecycle is not connected. Generated widgets still render on the canvas overlay. This view will list surface IDs, revisions, and stale evidence once that manager exists.',
  },
  audit: {
    title: 'Audit',
    body: 'Audit history is not connected. Broker decisions still write to the audit log. This view will show those entries when a read command exists. Secrets and model chain-of-thought stay out of this surface.',
  },
  settings: {
    title: 'Settings',
    body: 'Settings are not connected. The sidebar cannot edit the config file, route a request, or hold a credential. Typed settings will land here through backend commands.',
  },
};

// ---- Settings panel state (providers + roles administration) ----

/// One assignable role's panel state: chosen provider, the models its
/// discovery returned (or why discovery failed), and the chosen model.
export type RolePanelState = {
  provider: string;
  models: { id: string; name: string | null }[];
  model: string;
  discoveryError: string | null;
};

export type SettingsForm = {
  open: boolean;
  // Add-provider form: pick from catalog, endpoint auto-fills, enter key.
  catalogId: string;
  providerEndpoint: string;
  providerKey: string;
  // Bulk assignment: apply one provider/model to a whole group of roles.
  bulkGroup: string;
  bulkProvider: string;
  bulkModels: { id: string; name: string | null }[];
  bulkModel: string;
  bulkDiscoveryError: string | null;
  // Role assignment: one row per role from the backend's roles catalog.
  roles: Record<string, RolePanelState>;
  error: string | null;
  busy: boolean;
};

export const settingsForm: SettingsForm = {
  open: false,
  catalogId: '',
  providerEndpoint: '',
  providerKey: '',
  bulkGroup: 'specialists',
  bulkProvider: '',
  bulkModels: [],
  bulkModel: '',
  bulkDiscoveryError: null,
  roles: {},
  error: null,
  busy: false,
};

export function roleState(role: string): RolePanelState {
  let state = settingsForm.roles[role];
  if (!state) {
    state = { provider: '', models: [], model: '', discoveryError: null };
    settingsForm.roles[role] = state;
  }
  return state;
}

/// Provider records for the settings panel (from the last sidebar status).
/// Mirrors SidebarStatus.providers; kept separate so the panel renders even
/// when the status poll fails.
export let settingsProviders: SidebarStatus['providers'] = [];

export function updateSettingsProviders(providers: SidebarStatus['providers']): void {
  settingsProviders = providers;
}

/// The static provider catalog (OpenRouter etc.) fetched from the backend.
export let providerCatalog: { id: string; label: string; endpoint: string; kind: string; tier: string }[] = [];

export function updateProviderCatalog(catalog: typeof providerCatalog): void {
  providerCatalog = catalog;
}

/// The assignable roles (chat, verification, surface, one per specialist
/// domain) fetched from the backend so both sides agree on the list.
export let rolesCatalog: { id: string; label: string; detail: string; fit: string }[] = [];

export function updateRolesCatalog(roles: typeof rolesCatalog): void {
  rolesCatalog = roles;
}

function healthClass(health: string): string {
  switch (health) {
    case 'Healthy': return 'graph-h-healthy';
    case 'Degraded': return 'graph-h-degraded';
    case 'Unhealthy': return 'graph-h-unhealthy';
    case 'Stale': return 'graph-h-stale';
    default: return 'graph-h-unknown';
  }
}

function layoutGraphNodes(nodes: GraphNode[]): Map<string, { x: number; y: number }> {
  const positions = new Map<string, { x: number; y: number }>();
  const byLayer = new Map<string, GraphNode[]>();
  for (const node of nodes) {
    const list = byLayer.get(node.layer) ?? [];
    list.push(node);
    byLayer.set(node.layer, list);
  }
  const specialistRow1: GraphNode[] = [];
  const specialistRow2: GraphNode[] = [];
  const specNodes = byLayer.get('specialist') ?? [];
  specNodes.forEach((n, i) => { (i < 6 ? specialistRow1 : specialistRow2).push(n); });

  const layoutRow = (rowNodes: GraphNode[], y: number) => {
    const gap = 8;
    const totalW = rowNodes.length * NODE_W + (rowNodes.length - 1) * gap;
    const startX = (GRAPH_W - totalW) / 2;
    rowNodes.forEach((n, i) => {
      positions.set(n.id, { x: startX + i * (NODE_W + gap) + NODE_W / 2, y });
    });
  };

  for (const [layer, layerNodes] of byLayer) {
    if (layer === 'specialist') continue;
    const y = GRAPH_LAYER_Y[layer] ?? 140;
    layoutRow(layerNodes, y);
  }
  if (specialistRow1.length) layoutRow(specialistRow1, GRAPH_LAYER_Y.specialist);
  if (specialistRow2.length) layoutRow(specialistRow2, GRAPH_LAYER_Y.specialist + 34);
  return positions;
}

function computeActiveNodeIds(flight: FlightProgress | null, fallbackActive: string[]): Set<string> {
  if (!flight) return new Set(fallbackActive);
  if (flight.activeNodeIds.length) return new Set(flight.activeNodeIds);
  switch (flight.phase) {
    case 'planning':
      return new Set(['facade', 'coordinator', 'planner', 'gateway']);
    case 'gathering':
      return new Set(['coordinator', 'broker', ...flight.specialists, 'graph']);
    case 'composing':
      return new Set(['coordinator', 'gateway', 'composer']);
    case 'verifying':
      return new Set(['coordinator', 'verifier', 'gateway']);
    case 'policycheck':
      return new Set(['coordinator', 'broker', 'guardian']);
  }
}

function renderGraph(graph: SystemGraphSnapshot | null, flight: FlightProgress | null, escapeHtml: (value: string) => string): string {
  if (!graph || !graph.nodes.length) {
    return `<div class="graph-empty" role="status">System graph loading…</div>`;
  }
  const positions = layoutGraphNodes(graph.nodes);
  const nodePos = (id: string) => positions.get(id);

  const activeIds = computeActiveNodeIds(flight, graph.activeNodeIds);
  const isFlight = Boolean(flight);

  const edgeEls = graph.edges.map((edge) => {
    const from = nodePos(edge.from);
    const to = nodePos(edge.to);
    if (!from || !to) return '';
    const isActive = activeIds.has(edge.from) && activeIds.has(edge.to);
    return `<line class="graph-edge${isActive ? ' graph-edge-active' : ''}" x1="${from.x}" y1="${from.y}" x2="${to.x}" y2="${to.y}"/>`;
  }).filter(Boolean);

  const nodeEls = graph.nodes.map((node) => {
    const pos = nodePos(node.id);
    if (!pos) return '';
    const hc = healthClass(node.health);
    const active = activeIds.has(node.id) ? ' graph-node-active' : '';
    const x = pos.x - NODE_W / 2;
    const y = pos.y - NODE_H / 2;
    const nodeDescription = `${node.label} · ${node.health}${node.detail ? ` · ${node.detail}` : ''}`;
    return `<g class="graph-node ${hc}${active}" data-graph-id="${escapeHtml(node.id)}" tabindex="0" role="img" aria-label="${escapeHtml(nodeDescription)}">
      <rect class="graph-node-bg" x="${x}" y="${y}" width="${NODE_W}" height="${NODE_H}" rx="4"/>
      <text class="graph-node-label" x="${pos.x}" y="${pos.y + 1}" text-anchor="middle" dominant-baseline="central">${escapeHtml(node.label)}</text>
      <title>${escapeHtml(nodeDescription)}</title>
    </g>`;
  }).filter(Boolean);

  const phaseClass = isFlight ? ' graph-active' : '';

  return `<svg class="system-graph${phaseClass}" viewBox="0 0 ${GRAPH_W} ${GRAPH_H}" role="group" aria-label="Aios system topology">
    <g class="graph-edges" aria-hidden="true">${edgeEls.join('')}</g>
    <g class="graph-nodes">${nodeEls.join('')}</g>
  </svg>`;
}

function renderRailButton(id: SectionId, active: SectionId): string {
  const isActive = id === active;
  return `<button type="button" class="rail-btn${isActive ? ' active' : ''}" data-section="${id}" title="${SECTION_LABELS[id]}" aria-label="${SECTION_LABELS[id]}" aria-current="${isActive ? 'page' : 'false'}">${RAIL_ICONS[id]}</button>`;
}

function renderIconRail(view: SidebarView, backendOk: boolean): string {
  return `<nav class="icon-rail" aria-label="Aios">
    <div class="rail-top">
      <div class="rail-brand" title="Aios" aria-hidden="true">A</div>
    </div>
    <div class="rail-sections">
      <div class="rail-group" role="group" aria-label="Conversation">${renderRailButton('chat', view.section)}</div>
      <div class="rail-group" role="group" aria-label="System">${renderRailButton('providers', view.section)}${renderRailButton('roles', view.section)}${renderRailButton('surfaces', view.section)}</div>
      <div class="rail-group" role="group" aria-label="Administration">${renderRailButton('audit', view.section)}${renderRailButton('settings', view.section)}</div>
    </div>
    <div class="rail-status" aria-label="Status">
      <span class="rail-status-text" title="Backend ${backendOk ? 'ready' : 'not ready'}">${backendOk ? 'ON' : 'OFF'}</span>
    </div>
  </nav>`;
}

function graphReadout(graph: SystemGraphSnapshot | null, flight: FlightProgress | null): string {
  if (!graph) return '';
  const healthParts = graph.healthCounts.map(([state, count]) => `${count} ${state.toLowerCase()}`).join(', ');
  const phaseLabel = flight ? ` · ${flight.label.toLowerCase()}` : graph.phase !== 'idle' ? ` · ${graph.phase}` : '';
  return `<p class="graph-readout">${graph.totalNodes} nodes${healthParts ? ' · ' + healthParts : ''}${phaseLabel}</p>`;
}

function renderWorkbench(view: SidebarView, escapeHtml: (value: string) => string): string {
  if (!view.requestInFlight) return '';
  const kicker = view.flightProgress ? escapeHtml(view.flightProgress.label) : 'In flight';
  const detail = view.flightProgress
    ? escapeHtml(view.flightProgress.detail)
    : 'Waiting on the backend to gather evidence and compose a response.';
  return `<div class="workbench" role="status">
    <div class="workbench-kicker">${kicker}</div>
    <p class="workbench-copy">${detail}</p>
  </div>`;
}

function renderInspector(section: SectionId, escapeHtml: (value: string) => string): string {
  if (section === 'chat') return '';
  if (section === 'settings') return renderSettingsModal(escapeHtml);
  const copy = INSPECTOR[section];
  return `<section class="inspector" aria-label="${escapeHtml(copy.title)}">
    <header class="inspector-head">
      <div>
        <div class="inspector-kicker">Unavailable</div>
        <h2 class="inspector-title">${escapeHtml(copy.title)}</h2>
      </div>
      <button type="button" class="inspector-dismiss" data-dismiss-inspector>Close</button>
    </header>
    <p class="inspector-body">${escapeHtml(copy.body)}</p>
  </section>`;
}

// ---- Settings modal (provider + role administration) ----
//
// The settings icon (5th below chat) opens this overlay. It covers the
// sidebar content (graph + chat stay mounted underneath, just visually
// covered) and contains provider management and role assignment. API keys
// are write-only: they go in and are never returned to the frontend.

function renderSettingsModal(escapeHtml: (value: string) => string): string {
  return `<div class="settings-overlay" role="dialog" aria-modal="true" aria-label="Settings">
    <section class="settings-modal">
      <header class="settings-modal-head">
        <h2 class="settings-modal-title">Settings</h2>
        <button type="button" class="settings-modal-close" data-dismiss-inspector aria-label="Close settings">×</button>
      </header>
      <div class="settings-modal-body">${renderProvidersAndRolesPanel(escapeHtml)}</div>
    </section>
  </div>`;
}

function renderProvidersAndRolesPanel(escapeHtml: (value: string) => string): string {
  const providers = settingsProviders;
  const rows = providers.length
    ? providers.map((p) => `<div class="settings-provider" data-provider-id="${escapeHtml(p.id)}">
        <div class="settings-provider-main">
          <span class="settings-provider-id">${escapeHtml(p.id)}</span>
        </div>
        <div class="settings-provider-meta">
          <span class="${p.health === 'Healthy' ? 'settings-ok' : p.health === 'Unhealthy' ? 'settings-bad' : 'settings-warn'}">${escapeHtml(p.health)}</span>
          <span>${p.credentialConfigured ? 'key set' : 'no key'}</span>
          <button type="button" class="settings-key-btn" data-set-key="${escapeHtml(p.id)}">Set key</button>
          <button type="button" class="settings-remove-btn" data-remove-provider="${escapeHtml(p.id)}">Remove</button>
        </div>
      </div>`).join('')
    : '<p class="settings-empty">No providers configured. Add one below.</p>';

  // Custom dropdown (hidden input + trigger + listbox): native selects cannot
  // be styled on WebKitGTK and their popups ignore the color scheme entirely.
  const aiosSelect = (
    name: string,
    options: { value: string; label: string }[],
    selected: string,
    attrs = '',
  ): string => {
    const current = options.find((o) => o.value === selected);
    const listItems = options.map((o) =>
      `<li role="option" data-value="${escapeHtml(o.value)}" aria-selected="${o.value === selected}" class="${o.value === selected ? 'is-selected' : ''}${o.value ? '' : 'is-placeholder'}">${escapeHtml(o.label)}</li>`).join('');
    return `<div class="aios-select${current ? '' : ' is-placeholder'}" ${attrs}>
      <input type="hidden" name="${escapeHtml(name)}" value="${escapeHtml(selected)}">
      <button type="button" class="aios-select-trigger" aria-haspopup="listbox" aria-expanded="false">
        <span class="aios-select-value">${escapeHtml(current?.label ?? options[0]?.label ?? '')}</span>
        <svg class="aios-select-caret" viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 4.5 6 8l3.5-3.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </button>
      <ul class="aios-select-list" role="listbox" hidden>${listItems}</ul>
    </div>`;
  };

  const providerChoices = () =>
    [{ value: '', label: 'choose provider…' }]
      .concat(providers.map((p) => ({ value: p.id, label: p.id })));

  const modelChoices = (models: { id: string; name: string | null }[], selected: string) => {
    const choices = models.map((m) => ({ value: m.id, label: m.name ?? m.id }));
    // Keep an assigned-but-not-yet-discovered model selectable instead of
    // falling back to the placeholder.
    if (selected && !choices.some((c) => c.value === selected)) {
      choices.unshift({ value: selected, label: selected });
    }
    return [{ value: '', label: 'choose model…' }].concat(choices);
  };

  const roleRow = (role: { id: string; label: string; detail: string; fit: string }) => {
    const state = roleState(role.id);
    const discoveryNote = state.discoveryError
      ? `<p class="settings-role-error" role="alert">${escapeHtml(state.discoveryError)}</p>`
      : '';
    return `
    <form class="settings-role" data-role="${escapeHtml(role.id)}">
      <h3 class="settings-form-title">${escapeHtml(role.label)}</h3>
      <p class="settings-role-detail">${escapeHtml(role.detail)}</p>
      <p class="settings-role-fit"><span>Good fit</span>${escapeHtml(role.fit)}</p>
      <label class="settings-field">
        <span>Provider</span>
        ${aiosSelect('provider', providerChoices(), state.provider, `data-role-provider="${escapeHtml(role.id)}"`)}
      </label>
      <label class="settings-field">
        <span>Model</span>
        ${aiosSelect('model', modelChoices(state.models, state.model), state.model)}
      </label>
      ${discoveryNote}
      <button type="submit" class="settings-submit" ${settingsForm.busy ? 'disabled' : ''}>Assign</button>
    </form>`;
  };

  return `<div class="settings-tab-body">
    <section class="settings-section">
      <header class="settings-section-head">
        <h2 class="settings-section-title">Providers</h2>
        <p class="settings-section-hint">Endpoints that can serve models. Health updates live.</p>
      </header>
      <div class="settings-list">${rows}</div>
      <form class="settings-form" id="provider-form">
        <h3 class="settings-form-title">Add provider</h3>
        <label class="settings-field">
          <span>Provider</span>
          ${aiosSelect('catalog_id', [{ value: '', label: 'choose provider…' }].concat(providerCatalog.map((c) => ({ value: c.id, label: c.label }))), settingsForm.catalogId, 'data-catalog-select')}
        </label>
        <label class="settings-field">
          <span>Endpoint</span>
          <input name="endpoint" type="url" placeholder="auto-filled from provider" required value="${escapeHtml(settingsForm.providerEndpoint)}" readonly/>
        </label>
        <label class="settings-field">
          <span>API key</span>
          <input name="api_key" type="password" placeholder="sk-or-..." required/>
        </label>
        <button type="submit" class="settings-submit" ${settingsForm.busy ? 'disabled' : ''}>Add provider</button>
      </form>
    </section>
    <section class="settings-section">
      <header class="settings-section-head">
        <h2 class="settings-section-title">Roles</h2>
        <p class="settings-section-hint">Each role runs its own model. Assign providers per role below.</p>
      </header>
      ${providerIdsHint(providers.length)}
      <form class="settings-bulk" id="bulk-form">
        <p class="settings-bulk-hint">Apply one model to many roles at once. Individual roles keep their own assignment afterwards.</p>
        <label class="settings-field">
          <span>Group</span>
          ${aiosSelect('group', [
            { value: 'specialists', label: 'All specialists' },
            { value: 'all', label: 'All roles' },
          ], settingsForm.bulkGroup, 'data-bulk-group')}
        </label>
        <label class="settings-field">
          <span>Provider</span>
          ${aiosSelect('provider', providerChoices(), settingsForm.bulkProvider, 'data-bulk-provider')}
        </label>
        <label class="settings-field">
          <span>Model</span>
          ${aiosSelect('model', modelChoices(settingsForm.bulkModels, settingsForm.bulkModel), settingsForm.bulkModel)}
        </label>
        ${settingsForm.bulkDiscoveryError ? `<p class="settings-role-error" role="alert">${escapeHtml(settingsForm.bulkDiscoveryError)}</p>` : ''}
        <button type="submit" class="settings-submit" ${settingsForm.busy ? 'disabled' : ''}>Assign group</button>
      </form>
      <div class="settings-roles-grid">${rolesCatalog.map(roleRow).join('')}</div>
      ${settingsForm.error ? `<div class="settings-error" role="alert">${escapeHtml(settingsForm.error)}</div>` : ''}
    </section>
  </div>`;
}

function providerIdsHint(providerCount: number): string {
  return providerCount ? '' : '<p class="settings-empty">Add a provider above first.</p>';
}

function renderSystemFeedback(view: SidebarView, escapeHtml: (value: string) => string): string {
  const backendOk = view.status?.backendStatus.ready ?? false;
  const route = view.status?.currentRoute;
  const chatRoute = view.status?.chatRoute;
  const connectivity = view.status?.connectivity ?? 'Unknown';
  const fault = Boolean(view.status?.backendStatus.error ?? view.status?.routeError ?? view.statusError);
  const phase = fault
    ? 'Fault'
    : !backendOk
    ? 'Starting'
    : view.flightProgress
    ? view.flightProgress.label
    : view.requestInFlight
    ? 'In flight'
    : 'Ready';
  const phaseClass = fault
    ? 'phase-fault'
    : !backendOk
    ? 'phase-starting'
    : view.requestInFlight || Boolean(view.flightProgress)
    ? 'phase-inflight'
    : 'phase-ready';
  const routeLabel = route ? route.model : 'No route';
  const routeDetail = route ? route.provider : connectivity === 'Unknown' ? 'Unknown' : 'unassigned';
  const chatLabel = chatRoute ? chatRoute.model : 'No route';
  const chatDetail = chatRoute ? chatRoute.provider : 'unassigned';
  return `<section class="system-feedback" aria-label="System instrument" data-phase="${phase.toLowerCase().replace(' ', '-')}">
    <header class="instrument-head">
      <div class="instrument-phase">
        <span class="phase ${phaseClass}">${phase}</span>
        <span class="phase-dot" aria-hidden="true"></span>
      </div>
      <span class="instrument-conn" title="Connectivity">${escapeHtml(connectivity)}</span>
    </header>
    <div class="instrument-route" title="${route ? escapeHtml(`surface · ${route.provider} / ${route.model}`) : 'No surface model route'}">
      <span class="route-model">${escapeHtml(routeLabel)}</span>
     <span class="route-provider">${escapeHtml(`surface · ${routeDetail}`)}</span>
      ${route?.reducedConfidence ? '<span class="route-flag">reduced confidence</span>' : ''}
    </div>
    <div class="instrument-route" title="${chatRoute ? escapeHtml(`chat · ${chatRoute.provider} / ${chatRoute.model}`) : 'No chat model route'}">
      <span class="route-model">${escapeHtml(chatLabel)}</span>
      <span class="route-provider">${escapeHtml(`chat · ${chatDetail}`)}</span>
      ${chatRoute?.reducedConfidence ? '<span class="route-flag">reduced confidence</span>' : ''}
    </div>
    ${renderGraph(view.graph, view.flightProgress, escapeHtml)}
    ${graphReadout(view.graph, view.flightProgress)}
    ${renderWorkbench(view, escapeHtml)}
  </section>`;
}

function renderAlert(view: SidebarView, escapeHtml: (value: string) => string): string {
  const detail = view.status?.backendStatus.error
    ?? view.status?.routeError
    ?? view.statusError
    ?? view.graphError;
  if (!detail) return '';
  return `<div class="instrument-alert" role="alert">${escapeHtml(detail)}</div>`;
}

function messageState(message: SidebarMessage): MessageState {
  if (message.state) return message.state;
  return message.role === 'assistant' ? 'complete' : 'complete';
}

function renderMessage(message: SidebarMessage, escapeHtml: (value: string) => string): string {
  const state = messageState(message);
  const label = message.role === 'user' ? 'You' : state === 'pending' ? 'Aios · waiting' : state === 'failed' ? 'Aios · failed' : 'Aios';
  const retry = state === 'failed'
    ? '<button type="button" class="message-retry" data-retry>Retry last request</button>'
    : '';
  const evidence = message.evidence?.length
    ? `<details class="evidence-details"><summary>Specialist evidence (${message.evidence.length})</summary>${message.evidence.map((item) => `<div class="evidence-item"><strong>${escapeHtml(item.tool)}</strong><p>${escapeHtml(item.text)}</p></div>`).join('')}</details>`
    : '';
  return `<article class="message ${message.role} is-${state}">
    <div class="message-label">${label}</div>
    <div class="message-body">
      <p>${escapeHtml(message.text)}</p>
      ${evidence}
      ${retry}
    </div>
  </article>`;
}

function renderComposer(view: SidebarView): string {
  const hint = view.requestInFlight
    ? 'Waiting for the backend. Send is paused until this request finishes.'
    : '<kbd>Enter</kbd> to send · <kbd>Shift</kbd>+<kbd>Enter</kbd> for a new line';
  return `<form class="prompt-form" id="prompt-form">
    <label class="sr-only" for="prompt">Ask Aios</label>
    <div class="prompt-field">
      <textarea id="prompt" rows="1" placeholder="Ask Aios about your system..."></textarea>
      <button type="submit" class="prompt-send" aria-label="Send" disabled>
        <svg viewBox="0 0 18 18" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M4 9l10-5-3 10-2.5-4.5z"/><path d="M4 9l4.5.5"/></svg>
      </button>
    </div>
    <div class="prompt-hint">${hint}</div>
  </form>`;
}

export function isSectionId(value: string | undefined): value is SectionId {
  return value === 'chat' || value === 'providers' || value === 'roles' || value === 'surfaces' || value === 'audit' || value === 'settings';
}

export function renderSidebar(view: SidebarView, escapeHtml: (value: string) => string): string {
  const backendOk = view.status?.backendStatus.ready ?? false;
  return `<main class="app-shell sidebar-only">
    <aside class="sidebar">
      ${renderIconRail(view, backendOk)}
      <div class="sidebar-content">
        ${renderSystemFeedback(view, escapeHtml)}
        ${renderAlert(view, escapeHtml)}
        ${renderInspector(view.section, escapeHtml)}
        <section class="chat" aria-live="polite" aria-busy="${view.requestInFlight ? 'true' : 'false'}">${view.messages.map((message) => renderMessage(message, escapeHtml)).join('')}</section>
        ${renderComposer(view)}
      </div>
    </aside>
  </main>`;
}
