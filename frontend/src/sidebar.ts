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

export function renderSidebar(
  messages: SidebarMessage[],
  escapeHtml: (value: string) => string,
  status: SidebarStatus | null,
  statusError: string | null,
): string {
  const currentRoute = status?.currentRoute;
  const providerHealth = status?.providers.length
    ? status.providers.every((provider) => provider.health === 'Healthy') ? 'Healthy' : 'Attention'
    : 'No providers';
  const readiness = status?.backendStatus.ready ? 'Ready' : 'Starting';
  const statusDetail = status?.backendStatus.error ?? status?.routeError ?? statusError;
  return `<main class="app-shell sidebar-only">
    <aside class="sidebar">
      <div class="brand-row"><div class="brand-mark">A</div><div><div class="brand-name">Aios</div><div class="brand-status"><span class="status-dot"></span> System assistant</div></div></div>
      <section class="system-status" aria-label="Aios system status">
        <div class="status-heading"><span>System status</span><strong class="status-value ${readiness === 'Ready' ? 'status-good' : 'status-waiting'}">${escapeHtml(readiness)}</strong></div>
        <div class="status-grid"><div><span>Connectivity</span><strong>${escapeHtml(status?.connectivity ?? 'Starting')}</strong></div><div><span>Providers</span><strong>${escapeHtml(providerHealth)}</strong></div></div>
        ${currentRoute ? `<div class="status-route"><span>Active route</span><strong>${escapeHtml(currentRoute.provider)} / ${escapeHtml(currentRoute.model)}</strong></div>` : ''}
        ${statusDetail ? `<div class="status-error" role="status">${escapeHtml(statusDetail)}</div>` : ''}
      </section>
      <div class="conversation-label">Conversation</div>
      <section class="chat" aria-live="polite">${messages.map((message) => `<article class="message ${message.role}"><div class="message-label">${message.role === 'user' ? 'You' : 'Aios'}</div><p>${escapeHtml(message.text)}</p>${message.evidence?.length ? `<details class="evidence-details"><summary>Specialist evidence (${message.evidence.length})</summary>${message.evidence.map((item) => `<div class="evidence-item"><strong>${escapeHtml(item.tool)}</strong><p>${escapeHtml(item.text)}</p></div>`).join('')}</details>` : ''}</article>`).join('')}</section>
      <form class="prompt-form" id="prompt-form"><label class="sr-only" for="prompt">Ask Aios</label><textarea id="prompt" rows="3" placeholder="Ask Aios about your system..."></textarea><button type="submit">Send <span>↵</span></button></form>
    </aside>
  </main>`;
}
