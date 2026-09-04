const path = require('node:path');

const root = path.resolve(__dirname, '..');
const appBinaryPath = process.env.AIOS_APP_BIN || path.join(root, 'src-tauri', 'target', 'debug', 'aios-tauri');

exports.config = {
  runner: 'local',
  // The deterministic suite is the default CI-safe regression. The live
  // OpenRouter suite opts in explicitly through AIOS_UI_SPEC because it
  // consumes a real credential and must never run in parallel with it.
  specs: [process.env.AIOS_UI_SPEC || path.join(root, 'tests', 'wdio', 'live-surfaces.e2e.cjs')],
  maxInstances: 1,
  services: [['@wdio/tauri-service', {
    appBinaryPath,
    driverProvider: 'embedded',
    embeddedPort: 4445,
  }]],
  capabilities: [{
    browserName: 'tauri',
    'tauri:options': { application: appBinaryPath },
  }],
  logLevel: 'warn',
  waitforTimeout: 10_000,
  connectionRetryTimeout: 90_000,
  connectionRetryCount: 1,
  framework: 'mocha',
  mochaOpts: { ui: 'bdd', timeout: 90_000 },
};
