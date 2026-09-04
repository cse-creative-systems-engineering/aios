const assert = require('node:assert/strict');

const themes = [
  ['generate a surface displaying the disk usage please', 'disk'],
  ['generate a surface displaying the memory usage please', 'memory'],
  ['generate a surface displaying the cpu usage please', 'cpu'],
  ['generate a surface for network status please', 'network'],
  ['how healthy is the system graph', 'health'],
];

async function withWindowContaining(selector, action) {
  const deadline = Date.now() + 30_000;
  let lastHandles = [];
  while (Date.now() < deadline) {
    lastHandles = await browser.getWindowHandles();
    for (const handle of lastHandles) {
      await browser.switchToWindow(handle);
      if (await $(selector).isExisting()) return action(handle);
    }
    await browser.pause(200);
  }
  const summaries = [];
  for (const handle of lastHandles) {
    await browser.switchToWindow(handle);
    summaries.push((await $('body').getText()).slice(0, 600));
  }
  console.error(`Window text while waiting for ${selector}: ${JSON.stringify(summaries)}`);
  throw new Error(`No Tauri window contained ${selector}; saw ${lastHandles.length} handles`);
}

describe('Aios native desktop surface flow', () => {
  it('creates, accumulates, and closes generated surfaces through the real UI', async () => {
    const sidebar = await withWindowContaining('#prompt', async (handle) => handle);
    const prompt = await $('#prompt');
    assert.equal(await prompt.isDisplayed(), true, 'sidebar composer should be visible');

    let canvas;
    for (let index = 0; index < themes.length; index += 1) {
      const [request, key] = themes[index];
      await browser.switchToWindow(sidebar);
      const composer = await $('#prompt');
      await composer.setValue(request);
      await browser.execute(() => document.querySelector('#prompt-form').requestSubmit());

      const selector = `[data-aios-theme="${key}"]`;
      canvas = await withWindowContaining(selector, async (handle) => handle);
      await browser.switchToWindow(canvas);
      const cards = await $$('.aios-surface');
      const markers = await $$('[data-aios]');
      const closeButtons = await $$('[data-close]');
      assert.equal(cards.length, index + 1, `${key} should accumulate on the canvas`);
      assert.ok(markers.length > 0, `${key} should retain verified data bindings`);
      assert.equal(closeButtons.length, index + 1, `${key} should expose a close control`);
      assert.equal((await $$('.widget-grid')).length, 0, 'legacy fallback must never render');
    }

    await browser.switchToWindow(canvas);
    while (await $('[data-close]').isExisting()) {
      await $('[data-close]').click();
      await browser.pause(100);
    }
    assert.equal((await $$('.surface-host')).length, 0, 'all surfaces should close cleanly');
  });
});
