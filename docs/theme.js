// Blocking, before CSS: use saved preference before the first paint.
(() => {
  let preference = 'system';
  try { preference = localStorage.getItem('clipvault-theme') || 'system'; } catch (_) {}
  document.documentElement.dataset.theme = ['light', 'dark', 'system'].includes(preference) ? preference : 'system';
})();
