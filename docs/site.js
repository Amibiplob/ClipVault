'use strict';
const themeInput = document.querySelector('#theme');
themeInput.value = document.documentElement.dataset.theme;
themeInput.addEventListener('change', event => {
  const theme = ['system', 'light', 'dark'].includes(event.target.value) ? event.target.value : 'system';
  document.documentElement.dataset.theme = theme;
  try { localStorage.setItem('clipvault-theme', theme); } catch (_) {}
});
const api = window.ClipVaultRelease;
function applyRelease(data, cached = false) {
  const release = api.parse(data);
  document.querySelectorAll('[data-version]').forEach(node => { node.textContent = release.tag; });
  let count = 0;
  document.querySelectorAll('[data-format]').forEach(link => {
    const asset = release.assets[link.dataset.format];
    link.removeAttribute('href');
    link.setAttribute('aria-disabled', 'true');
    link.textContent = 'Package not in this release';
    link.closest('article').querySelector('[data-size]').textContent = 'x86-64 · not in this release';
    if (asset) {
      link.href = asset.url;
      link.removeAttribute('aria-disabled');
      link.textContent = `Download .${link.dataset.format}`;
      link.closest('article').querySelector('[data-size]').textContent = `${(asset.size / 1024 / 1024).toFixed(1)} MiB · x86-64`;
      count++;
    }
  });
  const checksum = document.querySelector('#checksums');
  checksum.href = release.checksum || release.notes;
  checksum.textContent = release.checksum ? 'Download SHA-256 checksums ↗' : 'Check verification files on GitHub ↗';
  document.querySelector('#release-notes-link').href = release.notes;
  document.querySelector('#release-status').textContent = `${cached ? 'Last checked release' : 'Latest stable release'}: ${release.tag} · ${count} package${count === 1 ? '' : 's'} available${cached ? '. Refreshing from GitHub…' : '.'}`;
}
async function loadRelease() {
  let cached;
  try {
    cached = JSON.parse(localStorage.getItem('clipvault-release-cache'));
    if (Date.now() - cached.checked < 86400000 && cached.checked <= Date.now()) applyRelease(cached.data, true);
    else cached = null;
  } catch (_) { cached = null; }
  try {
    const response = await fetch(`https://api.github.com/repos/${api.repository}/releases/latest`, { signal: AbortSignal.timeout(7000), headers: { Accept: 'application/vnd.github+json' } });
    if (!response.ok) throw Error('GitHub unavailable');
    const data = await response.json();
    applyRelease(data);
    try { localStorage.setItem('clipvault-release-cache', JSON.stringify({ data, checked: Date.now() })); } catch (_) {}
  } catch (_) {
    document.querySelector('#release-status').textContent = cached ? 'GitHub is unavailable. Showing release data checked within the last 24 hours; confirm availability on GitHub Releases.' : 'Release information is unavailable right now. Find published packages on GitHub Releases.';
  }
}
loadRelease();
