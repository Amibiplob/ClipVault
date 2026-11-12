// Static/runtime logic checks only. This is not browser layout or navigation proof.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const docs = path.join(__dirname, '..', 'docs');
const source = fs.readFileSync(path.join(docs, 'site.js'), 'utf8');
const html = fs.readFileSync(path.join(docs, 'index.html'), 'utf8');
const config = JSON.parse(fs.readFileSync(path.join(docs, 'release.json')));
assert.equal(config.version, '2.1.0');
for (const name of ['assets/clipvault.svg', 'history.png', 'style.css', 'site.js', 'release.json']) assert(fs.existsSync(path.join(docs, name)), name);
const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map(m => m[1]);
assert.equal(ids.length, new Set(ids).size, 'unique ids');
for (const [, id] of html.matchAll(/href="#([^"]+)"/g)) assert(ids.includes(id), `missing anchor ${id}`);
assert(html.includes('<summary>') && html.includes('aria-label="Website theme"'));
assert(!/sudo apt (update|install clipvault)/.test(html), 'no invented APT repo instructions');
async function scenario(data, fail = false) {
  const root = { dataset: {} };
  const theme = { addEventListener: (_, f) => { theme.change = f; } };
  const status = { textContent: '' };
  const labels = [{ textContent: '' }];
  const links = ['deb', 'AppImage', 'rpm'].map(format => ({ dataset: { format }, href: config.releases }));
  const saved = new Map([['clipvault-theme', 'dark']]);
  const document = { documentElement: root, querySelector: s => s === '#theme' ? theme : status, querySelectorAll: s => s === '[data-version]' ? labels : links };
  const fetch = async url => { if (fail) throw Error('offline fixture'); return { ok: true, json: async () => url === 'release.json' ? config : data }; };
  await vm.runInNewContext(source, { document, fetch, AbortSignal, localStorage: { getItem: k => saved.get(k), setItem: (k, v) => saved.set(k, v) } });
  assert.equal(root.dataset.theme, 'dark');
  theme.change({ target: { value: 'light' } });
  assert.equal(saved.get('clipvault-theme'), 'light');
  theme.change({ target: { value: 'invalid' } });
  assert.equal(root.dataset.theme, 'system');
  return { links, status };
}
(async () => {
  let result = await scenario({}, true);
  assert(result.links.every(l => l.href === config.releases));
  assert(result.status.textContent.includes('unavailable'));
  result = await scenario({ tag_name: 'v2.1.0', assets: [{ name: 'clipvault_2.1.0-1_amd64.deb', browser_download_url: `${config.releases}/download/v2.1.0/clipvault_2.1.0-1_amd64.deb` }, { name: 'malicious.rpm', browser_download_url: 'https://example.invalid/not-clipvault.rpm' }] });
  assert(result.links[0].href.endsWith('.deb'));
  assert.equal(result.links[2].href, config.releases);
  result = await scenario({ tag_name: 'v2.1.0', draft: true, assets: [] });
  assert(result.links.every(l => l.href === config.releases));
  console.log('Website checks passed: local assets/anchors, remembered themes, offline fallback, stable release assets, foreign URL rejection, draft rejection. No browser evidence.');
})().catch(error => { console.error(error); process.exitCode = 1; });
