/* Shared browser/build/test release validation. No OS guessing or invented links. */
(function (global) {
  const repository = 'Amibiplob/ClipVault';
  const releases = `https://github.com/${repository}/releases`;
  function parse(data) {
    if (!data || data.draft || data.prerelease || !/^v\.?\d+\.\d+\.\d+$/.test(data.tag_name)) throw Error('No stable release');
    const version = data.tag_name.replace(/^v\.?/, '');
    const names = { deb: `clipvault_${version}-1_amd64.deb`, rpm: `clipvault-${version}-1.x86_64.rpm`, AppImage: `ClipVault-${version}-x86_64.AppImage` };
    const base = `${releases}/download/${data.tag_name}/`;
    const assets = {};
    for (const [format, name] of Object.entries(names)) {
      const asset = (data.assets || []).find(a => a.name === name && a.browser_download_url === base + name && Number.isSafeInteger(a.size) && a.size > 0 && (!a.state || a.state === 'uploaded'));
      if (asset) assets[format] = { url: asset.browser_download_url, size: asset.size };
    }
    const checksumName = `${data.tag_name}-SHA256SUMS`;
    const checksum = (data.assets || []).find(a => a.name === checksumName && a.browser_download_url === base + checksumName);
    return { tag: data.tag_name, version, assets, notes: `${releases}/tag/${data.tag_name}`, checksum: checksum?.browser_download_url || null };
  }
  const api = { parse, repository, releases };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else global.ClipVaultRelease = api;
})(typeof window !== 'undefined' ? window : globalThis);
