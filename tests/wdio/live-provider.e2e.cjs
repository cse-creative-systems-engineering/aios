const assert = require('node:assert/strict');

const apiKey = process.env.AIOS_LIVE_OPENROUTER_API_KEY;
if (!apiKey) throw new Error('AIOS_LIVE_OPENROUTER_API_KEY is required for the live-provider suite');

async function selectOption(box, value) {
  const trigger = await box.$('.aios-select-trigger');
  await browser.execute((element) => {
    element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }));
  }, trigger);
  const option = await box.$(`.aios-select-list li[data-value="${value}"]`);
  await option.waitForDisplayed({ timeout: 15_000 });
  await browser.execute((element) => {
    element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 }));
  }, option);
}

async function sidebarWindow() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    for (const handle of await browser.getWindowHandles()) {
      await browser.switchToWindow(handle);
      if (await $('#prompt').isExisting()) return handle;
    }
    await browser.pause(200);
  }
  throw new Error('the native sidebar window did not expose its chat composer');
}

async function assignRole(role, model) {
  let form = await $(`.settings-role[data-role="${role}"]`);
  let selects = await form.$$('.aios-select');
  await selectOption(selects[0], 'openrouter');
  await browser.waitUntil(async () => {
    form = await $(`.settings-role[data-role="${role}"]`);
    return (await form.$(`.aios-select-list li[data-value="${model}"]`).isExisting());
  }, { timeout: 45_000, timeoutMsg: `${role} did not retain the discovered free model` });
  form = await $(`.settings-role[data-role="${role}"]`);
  selects = await form.$$('.aios-select');
  await selectOption(selects[1], model);
  await browser.execute((element) => element.requestSubmit(), form);
  await browser.waitUntil(async () => {
    const input = await $(`.settings-role[data-role="${role}"] input[name="model"]`);
    return (await input.getValue()) === model;
  }, { timeout: 15_000, timeoutMsg: `${role} assignment was not saved` });
}

describe('Aios live OpenRouter journey', () => {
  it('adds OpenRouter, discovers a real free model, assigns roles, and completes chat', async () => {
    await sidebarWindow();
    await $('#prompt').waitForDisplayed({ timeout: 30_000 });
    await $('.rail-btn[data-section="settings"]').click();
    const providerForm = await $('#provider-form');
    await providerForm.waitForDisplayed({ timeout: 15_000 });
    await selectOption(await providerForm.$('[data-catalog-select]'), 'openrouter');
    assert.equal(await providerForm.$('input[name="endpoint"]').getValue(), 'https://openrouter.ai/api/v1');
    await providerForm.$('input[name="api_key"]').setValue(apiKey);
    await browser.execute((element) => element.requestSubmit(), providerForm);
    await browser.waitUntil(async () => (await $('[data-set-key="openrouter"]').isExisting()), {
      timeout: 30_000,
      timeoutMsg: 'OpenRouter was not added through the visible provider form',
    });
    assert.match(await $('.settings-list').getText(), /key set/, 'the UI should report a configured credential');
    assert.equal((await $('body').getText()).includes(apiKey), false, 'the API key must never be rendered');

    const chatForm = await $('.settings-role[data-role="chat"]');
    let selects = await chatForm.$$('.aios-select');
    await selectOption(selects[0], 'openrouter');
    await browser.waitUntil(async () => await browser.execute(() => {
      const input = document.querySelector('.settings-role[data-role="chat"] input[name="model"]');
      return input?.closest('.aios-select')?.querySelectorAll('li[data-value]').length ?? 0;
    }) > 1, {
      timeout: 45_000,
      timeoutMsg: 'OpenRouter model discovery did not populate the Chat role',
    });
    const modelValues = await browser.execute(() => {
      const input = document.querySelector('.settings-role[data-role="chat"] input[name="model"]');
      return Array.from(input?.closest('.aios-select')?.querySelectorAll('li[data-value]') ?? [])
        .map((item) => item.dataset.value)
        .filter(Boolean);
    });
    const freeModel = modelValues.find((id) => id.endsWith(':free'));
    assert.ok(freeModel, `OpenRouter discovery returned no free model: ${JSON.stringify(modelValues)}`);

    await assignRole('chat', freeModel);
    await assignRole('verification', freeModel);
    await assignRole('surface', freeModel);

    await $('.rail-btn[data-section="settings"]').click();
    const prompt = await $('#prompt');
    await prompt.waitForDisplayed({ timeout: 15_000 });
    await prompt.setValue('In one concise sentence, say hello and confirm you can inspect my system.');
    await browser.execute(() => document.querySelector('#prompt-form').requestSubmit());
    await browser.waitUntil(async () => {
      const messages = await $$('.message.assistant');
      return messages.length >= 2 && !(await messages[messages.length - 1].getText()).includes('Backend unavailable:');
    }, { timeout: 120_000, timeoutMsg: 'live OpenRouter chat did not return an assistant response' });
  });
});
