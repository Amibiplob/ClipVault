const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname, '..');
const output = path.join(root, 'target/site');
fs.mkdirSync(output, { recursive: true });
for (const name of ['index.html', 'site.js', 'theme.js', 'release-data.js', 'history.png', 'assets']) {
  fs.cpSync(path.join(root, 'docs', name), path.join(output, name), { recursive: true });
}
fs.writeFileSync(path.join(output, '.nojekyll'), '');
console.log('Static website: target/site (relative URLs support /ClipVault/ and custom domains)');
