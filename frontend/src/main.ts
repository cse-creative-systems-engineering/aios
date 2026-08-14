import '../index.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { currentMonitor, getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalPosition } from '@tauri-apps/api/dpi';

type EvidenceItem = { tool: string; text: string };
type Message = { role: 'user' | 'assistant'; text: string; evidence?: EvidenceItem[] };
type UiWidget =
  | { type: 'metricCard'; label: string; value: string; unit: string; status: string }
  | { type: 'statusList'; title: string; items: string[] }
  | { type: 'notice'; title: string; body: string };
type PromptResponse = { answer: string; evidence: EvidenceItem[]; widgets: UiWidget[] };

const currentWindow = getCurrentWindow();
const isCanvasWindow = currentWindow.label === 'canvas';
let widgets: UiWidget[] = [];
const messages: Message[] = [{
  role: 'assistant',
  text: 'I’m ready to investigate your system. Ask me what you would like to know.',
}];

function render(): void {
  const root = document.querySelector<HTMLDivElement>('#root');
  if (!root) return;
  root.innerHTML = isCanvasWindow ? renderCanvas() : renderSidebar();
  if (!isCanvasWindow) {
    document.querySelector<HTMLFormElement>('#prompt-form')?.addEventListener('submit', submitPrompt);
    document.querySelector<HTMLTextAreaElement>('#prompt')?.addEventListener('pointerdown', () => {
      void currentWindow.setFocus();
    });
  } else {
    document.querySelectorAll<HTMLButtonElement>('[data-dock]').forEach((button) => {
      button.addEventListener('click', () => void dockPanel(button.dataset.dock as DockEdge));
    });
  }
}

function renderSidebar(): string {
  return `<main class="app-shell sidebar-only">
    <aside class="sidebar">
      <div class="brand-row"><div class="brand-mark">A</div><div><div class="brand-name">Aios</div><div class="brand-status"><span class="status-dot"></span> System assistant</div></div></div>
      <div class="conversation-label">Conversation</div>
      <section class="chat" aria-live="polite">${messages.map((message) => `<article class="message ${message.role}"><div class="message-label">${message.role === 'user' ? 'You' : 'Aios'}</div><p>${escapeHtml(message.text)}</p>${message.evidence?.length ? `<details class="evidence-details"><summary>Specialist evidence (${message.evidence.length})</summary>${message.evidence.map((item) => `<div class="evidence-item"><strong>${escapeHtml(item.tool)}</strong><p>${escapeHtml(item.text)}</p></div>`).join('')}</details>` : ''}</article>`).join('')}</section>
      <form class="prompt-form" id="prompt-form"><label class="sr-only" for="prompt">Ask Aios</label><textarea id="prompt" rows="3" placeholder="Ask Aios about your system..."></textarea><button type="submit">Send <span>↵</span></button></form>
    </aside>
  </main>`;
}

function renderCanvas(): string {
  return `<main class="canvas-window"><header class="canvas-header"><div><div class="eyebrow">Live evidence</div><h1>System overview</h1></div><div><div class="connection-state"><span class="status-dot"></span> Verified backend evidence</div><nav class="dock-actions" aria-label="Dock panel"><button data-dock="left">Left</button><button data-dock="right">Right</button><button data-dock="top">Top</button><button data-dock="bottom">Bottom</button></nav></div></header><div class="widget-grid">${widgets.map(renderWidget).join('')}</div></main>`;
}

type DockEdge = 'left' | 'right' | 'top' | 'bottom';

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

async function submitPrompt(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const input = document.querySelector<HTMLTextAreaElement>('#prompt');
  const text = input?.value.trim();
  if (!text) return;
  messages.push({ role: 'user', text });
  render();
  try {
    const response = await invoke<PromptResponse>('submit_prompt', { prompt: text });
    messages.push({ role: 'assistant', text: response.answer, evidence: response.evidence });

  } catch (error) {
    messages.push({ role: 'assistant', text: `Backend unavailable: ${String(error)}` });
  }
  render();
}

function renderWidget(widget: UiWidget): string {
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

if (isCanvasWindow) {
  void listen<PromptResponse>('canvas_response', async (event) => {
    widgets = event.payload.widgets;
    render();
    await currentWindow.show();
    await currentWindow.setFocus();
  });
}

render();
