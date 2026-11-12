'use strict';
const root = document.documentElement;
const themes = ['system', 'light', 'dark'];
const themeInput = document.querySelector('#theme');
function setTheme(value) {
  const theme = themes.includes(value) ? value : 'system';
  root.dataset.theme = theme;
  themeInput.value = theme;
  try { localStorage.setItem('clipvault-theme', theme); } catch (_) { /* private browser */ }
}
try { setTheme(localStorage.getItem('clipvault-theme')); } catch (_) { setTheme('system'); }
themeInput.addEventListener('change', e => setTheme(e.target.value));
async function release() {
  const status = document.querySelector('#release-status');
  try {
    const config = await fetch('release.json').then(r => { if (!r.ok) throw Error('config'); return r.json(); });
    document.querySelectorAll('[data-version]').forEach(node => { node.textContent = config.version; });
    const response = await fetch(`https://api.github.com/repos/${config.repository}/releases/latest`, { signal: AbortSignal.timeout(7000) });
    if (!response.ok) throw Error('release lookup');
    const data = await response.json();
    if (data.draft || data.prerelease || !/^v?\d+\.\d+\.\d+$/.test(data.tag_name)) throw Error('not stable');
    const base = `https://github.com/${config.repository}/releases/download/`;
    let count = 0;
    document.querySelectorAll('[data-format]').forEach(link => {
      const format = link.dataset.format;
      const asset = (data.assets || []).find(a => a.name.endsWith(`.${format}`) && a.browser_download_url.startsWith(base) && /^https:\/\/github\.com\//.test(a.browser_download_url));
      if (asset) {
        link.href = asset.browser_download_url;
        link.textContent = `↓ Download ${format} · ${data.tag_name}`;
        count++;
      }
    });
    status.textContent = `Latest published release: ${data.tag_name}. ${count ? 'Available package links are shown below.' : 'Check GitHub Releases for package availability.'} The source upgrade shown here is ${config.version}.`;
  } catch (_) {
    status.textContent = 'Release lookup unavailable. Use GitHub Releases below to see the packages actually published; v2.1.0 publication is not assumed.';
  }
}
release();
