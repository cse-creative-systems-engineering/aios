export type EvidenceItem = { tool: string; text: string };
export type SidebarMessage = {
  role: 'user' | 'assistant';
  text: string;
  evidence?: EvidenceItem[];
};

export function renderSidebar(
  messages: SidebarMessage[],
  escapeHtml: (value: string) => string,
): string {
  return `<main class="app-shell sidebar-only">
    <aside class="sidebar">
      <div class="brand-row"><div class="brand-mark">A</div><div><div class="brand-name">Aios</div><div class="brand-status"><span class="status-dot"></span> System assistant</div></div></div>
      <div class="conversation-label">Conversation</div>
      <section class="chat" aria-live="polite">${messages.map((message) => `<article class="message ${message.role}"><div class="message-label">${message.role === 'user' ? 'You' : 'Aios'}</div><p>${escapeHtml(message.text)}</p>${message.evidence?.length ? `<details class="evidence-details"><summary>Specialist evidence (${message.evidence.length})</summary>${message.evidence.map((item) => `<div class="evidence-item"><strong>${escapeHtml(item.tool)}</strong><p>${escapeHtml(item.text)}</p></div>`).join('')}</details>` : ''}</article>`).join('')}</section>
      <form class="prompt-form" id="prompt-form"><label class="sr-only" for="prompt">Ask Aios</label><textarea id="prompt" rows="3" placeholder="Ask Aios about your system..."></textarea><button type="submit">Send <span>↵</span></button></form>
    </aside>
  </main>`;
}
