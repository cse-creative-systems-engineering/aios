const assert = require('node:assert/strict');

const themes = [
  ['generate a surface displaying the disk usage please', 'disk'],
  ['generate a surface displaying the memory usage please', 'memory'],
  ['generate a surface displaying the cpu usage please', 'cpu'],
  ['generate a surface for network status please', 'network'],
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

async function selectOption(box, value) {
  const trigger = await box.$('.aios-select-trigger');
  // The desktop control intentionally handles pointerdown because WebKitGTK
  // can reroute a click in its frameless window. Trigger that native event
  // directly so this test exercises the same selection path.
  await browser.execute((element) => {
    element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }));
  }, trigger);
  const option = await box.$(`.aios-select-list li[data-value="${value}"]`);
  await option.waitForDisplayed({ timeout: 10_000 });
  await browser.execute((element) => {
    element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }));
  }, option);
}

async function assignRole(role, model) {
  let form = await $(`.settings-role[data-role="${role}"]`);
  await form.waitForDisplayed({ timeout: 10_000 });
  let selects = await form.$$('.aios-select');
  await selectOption(selects[0], 'openrouter');

  await browser.waitUntil(async () => {
    form = await $(`.settings-role[data-role="${role}"]`);
    return (await form.$(`.aios-select-list li[data-value="${model}"]`).isExisting());
  }, { timeout: 15_000, timeoutMsg: `${role} models were not discovered` });

  form = await $(`.settings-role[data-role="${role}"]`);
  selects = await form.$$('.aios-select');
  await selectOption(selects[1], model);
  await browser.execute((element) => element.requestSubmit(), form);
  await browser.waitUntil(async () => {
    const current = await $(`.settings-role[data-role="${role}"] input[name="model"]`);
    return (await current.getValue()) === model;
  }, { timeout: 10_000, timeoutMsg: `${role} assignment did not persist` });
}

describe('Aios native desktop surface flow', () => {
  it('onboards a provider, assigns role-specific models, chats, and creates surfaces', async () => {
    const sidebar = await withWindowContaining('#prompt', async (handle) => handle);
    const prompt = await $('#prompt');
    assert.equal(await prompt.isDisplayed(), true, 'sidebar composer should be visible');

    await $('.rail-btn[data-section="settings"]').click();
    const providerForm = await $('#provider-form');
    await providerForm.waitForDisplayed({ timeout: 10_000 });
    await selectOption(await providerForm.$('[data-catalog-select]'), 'openrouter');
    const endpoint = await providerForm.$('input[name="endpoint"]');
    await browser.waitUntil(async () => (await endpoint.getValue()).includes('127.0.0.1'), {
      timeout: 10_000,
      timeoutMsg: 'test provider endpoint was not filled from the catalog',
    });
    const testSecret = 'e2e-only-not-a-real-key';
    await providerForm.$('input[name="api_key"]').setValue(testSecret);
    await browser.execute((element) => element.requestSubmit(), providerForm);
    await browser.waitUntil(async () => (await $('[data-set-key="openrouter"]').isExisting()), {
      timeout: 15_000,
      timeoutMsg: 'provider was not added through the settings form',
    });
    assert.match(await $('.settings-list').getText(), /key set/, 'credential state should be visible');
    assert.equal((await $('body').getText()).includes(testSecret), false, 'credential must remain write-only');

    await assignRole('chat', 'stub-chat');
    await assignRole('verification', 'stub-verification');
    await assignRole('surface', 'stub-surface');

    await $('.rail-btn[data-section="settings"]').click();
    await $('#prompt').waitForDisplayed({ timeout: 10_000 });
    await $('#prompt').setValue('generate a surface displaying the disk usage please');
    await browser.execute(() => document.querySelector('#prompt-form').requestSubmit());
    try {
      await browser.waitUntil(async () => {
        const assistantMessages = await $$('.message.assistant');
        const last = assistantMessages[assistantMessages.length - 1];
        return last && (await last.getText()).includes('answered by stub-chat');
      }, {
        timeout: 30_000,
        timeoutMsg: 'chat did not complete through the assigned chat model',
      });
    } catch (error) {
      const messages = await $$('.message');
      const rendered = [];
      for (const message of messages) rendered.push(await message.getText());
      throw new Error(`chat did not complete through the assigned chat model; rendered messages: ${JSON.stringify(rendered)}; ${String(error)}`);
    }

    let canvas;
    for (let index = 0; index < themes.length; index += 1) {
      const [request, key] = themes[index];
      if (index === 0) {
        const selector = `[data-aios-theme="${key}"]`;
        canvas = await withWindowContaining(selector, async (handle) => handle);
        await browser.switchToWindow(canvas);
        assert.equal(await $(`[data-aios-model="stub-surface"]`).isExisting(), true, 'surface must use its assigned model');
        continue;
      }
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
    const cpuSurface = await $('[data-aios-theme="cpu"]');
    const cpuBinding = await cpuSurface.$('[data-aios*="cpu."]');
    assert.equal(await cpuBinding.isExisting(), true, 'CPU surface should declare a stable live projection key');
    const bindingKey = await cpuBinding.getAttribute('data-aios');
    assert.match(bindingKey ?? '', /^cpu\./, 'binding key should come from the live CPU projection');
    const cpuHost = await cpuSurface.$('..');
    const surfaceId = await cpuHost.getAttribute('data-surface-id');
    assert.ok(surfaceId, 'CPU surface should retain its backend-owned identity');
    const visualRevisionBefore = Number(await cpuHost.getAttribute('data-surface-revision'));
    assert.equal(visualRevisionBefore, 1, 'a new surface should start at visual revision one');
    const dataRevisionBefore = Number(await cpuHost.getAttribute('data-aios-data-revision'));
    assert.ok(Number.isInteger(dataRevisionBefore) && dataRevisionBefore >= 0, 'surface should expose a monotonic data revision');
    await browser.execute(async (key) => {
      window.dispatchEvent(new CustomEvent('aios-test-publish-state', {
        detail: { key, value: '77.77' },
      }));
    }, bindingKey);
    await browser.waitUntil(async () => (await cpuBinding.getText()) === '77.77', {
      timeout: 10_000,
      timeoutMsg: 'live state update did not replace the declared binding in place',
    });
    assert.equal(await cpuHost.getAttribute('data-surface-id'), surfaceId, 'live update must retain the surface identity');
    assert.ok(Number(await cpuHost.getAttribute('data-aios-data-revision')) > dataRevisionBefore, 'live update should advance data revision only');

    await browser.execute(() => { window.prompt = () => 'make the title neon yellow'; });
    await cpuHost.$('[data-edit]').click();
    await browser.waitUntil(async () => {
      const host = await $(`[data-surface-id="${surfaceId}"]`);
      return Number(await host.getAttribute('data-surface-revision')) === visualRevisionBefore + 1;
    }, {
      timeout: 20_000,
      timeoutMsg: 'surface edit did not produce a new visual revision',
    });
    const revisedHost = await $(`[data-surface-id="${surfaceId}"]`);
    assert.equal(await revisedHost.getAttribute('data-surface-id'), surfaceId, 'revision must retain the targeted surface identity');
    assert.equal((await $$('.surface-host')).length, themes.length, 'revision must not replace another surface');

    // Minimize is not close: it removes the desktop input region but retains
    // the exact backend-owned record. Restore is intentionally driven from
    // the resident sidebar, proving canvas visibility can recover without a
    // generic current-surface pointer.
    await revisedHost.$('[data-minimize]').click();
    await browser.waitUntil(async () => !(await $(`[data-surface-id="${surfaceId}"]`).isExisting()), {
      timeout: 10_000,
      timeoutMsg: 'minimized surface remained on the canvas',
    });
    await browser.switchToWindow(sidebar);
    await $('.rail-btn[data-section="surfaces"]').click();
    const record = await $(`[data-surface-record="${surfaceId}"]`);
    await record.waitForDisplayed({ timeout: 10_000 });
    assert.match(await record.getText(), /minimized/, 'sidebar should expose retained minimized lifecycle state');
    await record.$('[data-surface-show]').click();
    canvas = await withWindowContaining(`[data-surface-id="${surfaceId}"]`, async (handle) => handle);
    await browser.switchToWindow(canvas);
    const restoredHost = await $(`[data-surface-id="${surfaceId}"]`);
    assert.equal(Number(await restoredHost.getAttribute('data-surface-revision')), visualRevisionBefore + 1, 'restore must not revise the model-authored surface');

    // A user resize persists as lifecycle data, rather than a generated
    // template dimension. Reloading the canvas exercises restore from the
    // backend record after the webview loses its ephemeral JS state.
    const resize = await restoredHost.$('[data-resize]');
    const beforeSize = await restoredHost.getSize();
    await browser.execute((element) => {
      element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0, pointerId: 41, clientX: 10, clientY: 10 }));
      element.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, button: 0, pointerId: 41, clientX: 70, clientY: 45 }));
      element.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, button: 0, pointerId: 41, clientX: 70, clientY: 45 }));
    }, resize);
    await browser.waitUntil(async () => (await restoredHost.getSize()).width >= beforeSize.width + 50, {
      timeout: 10_000,
      timeoutMsg: 'surface resize did not change the visible presentation size',
    });
    await browser.refresh();
    const restartedHost = await $(`[data-surface-id="${surfaceId}"]`);
    await restartedHost.waitForDisplayed({ timeout: 15_000 });
    assert.ok((await restartedHost.getSize()).width >= beforeSize.width + 50, 'canvas restart must restore the user-selected size');

    while (await $('[data-close]').isExisting()) {
      await $('[data-close]').click();
      await browser.pause(100);
    }
    assert.equal((await $$('.surface-host')).length, 0, 'all surfaces should close cleanly');
  });
});
