export type EvidenceItem = { tool: string; text: string };
export type SidebarMessage = {
  role: 'user' | 'assistant';
  text: string;
  evidence?: EvidenceItem[];
};

export type SidebarStatus = {
  backendStatus: { ready: boolean; error: string | null };
  connectivity: string;
  currentRoute: {
    provider: string;
    model: string;
    connectivity: string;
    dataClassification: string;
    reducedConfidence: boolean;
  } | null;
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

type SectionId = 'chat' | 'providers' | 'roles' | 'surfaces' | 'audit' | 'settings';

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

function renderIconRail(backendOk: boolean, internetOk: boolean): string {
  const sections: SectionId[] = ['chat', 'providers', 'roles', 'surfaces', 'audit', 'settings'];
  return `<nav class="icon-rail" aria-label="Navigation">
    <div class="rail-top">
      <div class="rail-brand" title="Aios">A</div>
    </div>
    <div class="rail-sections">${sections.map((id) => `<button class="rail-btn${id === 'chat' ? ' active' : ''}" data-section="${id}" title="${SECTION_LABELS[id]}">${RAIL_ICONS[id]}</button>`).join('')}</div>
    <div class="rail-bottom">
      <span class="rail-dot${backendOk ? ' dot-good' : ' dot-bad'}" title="Backend ${backendOk ? 'ready' : 'not ready'}"></span>
      <span class="rail-dot${internetOk ? ' dot-good' : ' dot-bad'}" title="Internet ${internetOk ? 'connected' : 'unavailable'}"></span>
    </div>
  </nav>`;
}

function renderSystemFeedback(status: SidebarStatus | null, escapeHtml: (v: string) => string): string {
  const backendOk = status?.backendStatus.ready ?? false;
  const route = status?.currentRoute;
  const providers = status?.providers ?? [];
  const healthyProviders = providers.filter((p) => p.health === 'Healthy').length;
  const totalProviders = providers.length;
  const connectivity = status?.connectivity ?? 'Unknown';

  return `<section class="system-feedback" aria-label="System status">
    <div class="feedback-row feedback-system">
      <span class="feedback-label">System</span>
      <span class="feedback-value${backendOk ? ' value-good' : ' value-bad'}">${backendOk ? 'Ready' : 'Starting'}</span>
      <span class="feedback-detail">${escapeHtml(connectivity)}</span>
    </div>
    <div class="feedback-row feedback-model">
      <span class="feedback-label">Model</span>
      <span class="feedback-value">${route ? escapeHtml(route.model) : 'None'}</span>
      <span class="feedback-detail">${route ? escapeHtml(route.provider) : ''}</span>
    </div>
    <div class="feedback-row feedback-providers">
      <span class="feedback-label">Providers</span>
      <span class="feedback-value${healthyProviders === totalProviders ? ' value-good' : ' value-bad'}">${healthyProviders}/${totalProviders}</span>
      <span class="feedback-detail">healthy</span>
    </div>
    <div class="feedback-row feedback-specialist">
      <span class="feedback-label">Specialist</span>
      <span class="feedback-value value-idle">Idle</span>
      <span class="feedback-detail">waiting for task</span>
    </div>
    <div class="feedback-row feedback-surface">
      <span class="feedback-label">Surface</span>
      <span class="feedback-value value-idle">None</span>
      <span class="feedback-detail">no active surface</span>
    </div>
  </section>`;
}

export function renderSidebar(
  messages: SidebarMessage[],
  escapeHtml: (value: string) => string,
  status: SidebarStatus | null,
  statusError: string | null,
): string {
  const backendOk = status?.backendStatus.ready ?? false;
  const internetOk = status?.connectivity === 'Internet';
  const statusDetail = status?.backendStatus.error ?? status?.routeError ?? statusError;
  return `<main class="app-shell sidebar-only">
    <aside class="sidebar">
      ${renderIconRail(backendOk, internetOk)}
      <div class="sidebar-content">
        ${renderSystemFeedback(status, escapeHtml)}
        ${statusDetail ? `<div class="status-error" role="status">${escapeHtml(statusDetail)}</div>` : ''}
        <section class="chat" aria-live="polite">${messages.map((message) => `<article class="message ${message.role}"><div class="message-label">${message.role === 'user' ? 'You' : 'Aios'}</div><p>${escapeHtml(message.text)}</p>${message.evidence?.length ? `<details class="evidence-details"><summary>Specialist evidence (${message.evidence.length})</summary>${message.evidence.map((item) => `<div class="evidence-item"><strong>${escapeHtml(item.tool)}</strong><p>${escapeHtml(item.text)}</p></div>`).join('')}</details>` : ''}</article>`).join('')}</section>
        <form class="prompt-form" id="prompt-form"><label class="sr-only" for="prompt">Ask Aios</label><textarea id="prompt" rows="2" placeholder="Ask Aios about your system..."></textarea><button type="submit">Send <span>↵</span></button></form>
      </div>
    </aside>
  </main>`;
}
