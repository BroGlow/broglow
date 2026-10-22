const { mkdir, readFile, readdir, writeFile } = require('node:fs/promises');
const { dirname, resolve } = require('node:path');

const repository = resolve(__dirname, '../..');
const marker = '.broglow-build.json';
const kind = 'broglow-homepage-demo';
const pageFiles = [
  'index.html', 'styles.css', 'themes.css', 'theme.js', 'language.js',
  'app.js', 'app-preview.js', 'lamp.js', 'ar.js', 'ar.css',
];
const productFiles = [
  'assets/popover.html',
  ...['popover.css', 'popover.js', 'presentation.js', 'document.js'].map(name => `assets/popover/${name}`),
  ...['green', 'yellow', 'red', 'none'].map(color => `assets/icons/broglow-${color}.png`),
  'site/github.js', 'site/guide/assets/favicon.svg', 'site/assets/broglow.glb',
  ...['broglow-all-lights.jpg', 'popover-dark.png', 'signal-green.jpg', 'signal-yellow.jpg', 'signal-red.jpg']
    .map(name => `site/assets/photos/${name}`),
  'site/vendor/model-viewer-module-4.3.1.min.js',
  'site/vendor/qrcode-generator-2.0.4.js',
  'site/vendor/three-0.183.0/three.module.min.js',
  'site/vendor/three-0.183.0/three.core.min.js',
  ...['model-viewer', 'qrcode-generator', 'three', 'lit', 'lit-html'].map(name => `site/vendor/${name}-LICENSE`),
];

async function build(destination = resolve(repository, 'dist/homepage-demo')) {
  destination = resolve(destination);
  if ([__dirname, repository].includes(destination)) {
    throw new Error('Choose a dedicated build directory');
  }
  let entries;
  try { entries = await readdir(destination); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  if (entries?.length) {
    let receipt;
    try { receipt = JSON.parse(await readFile(resolve(destination, marker), 'utf8')); } catch {}
    if (receipt?.kind !== kind) throw new Error('Build directory is not empty and is not a BroGlow demo build');
  }

  // Only page paths change; shared UI and vendor files remain the shipped source bytes.
  const files = await Promise.all([
    ...pageFiles.map(async name => [name,
      (await readFile(resolve(__dirname, name), 'utf8')).replaceAll('../../', './broglow/')]),
    ...productFiles.map(async name => [`broglow/${name}`, await readFile(resolve(repository, name))]),
  ]);
  for (const [name, contents] of files) {
    const target = resolve(destination, name);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, contents);
  }
  const receipt = { kind, files: files.map(([name]) => name).sort() };
  await writeFile(resolve(destination, marker), `${JSON.stringify(receipt, null, 2)}\n`);
  return { destination, files: receipt.files };
}

module.exports = { build };

if (require.main === module) {
  build(process.argv[2]).then(({ destination, files }) => {
    console.log(`Built ${files.length} static files: ${destination}`);
  }).catch(error => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
