// Run with NODE_PATH pointing to a Sharp installation. SVGs are the authoring source.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
(async () => {
  for (const name of ['wipeout', 'hot-zone', 'heist', 'zombie']) {
    await sharp(fs.readFileSync(path.join(__dirname, `${name}.svg`)))
      .resize(300, 372).png()
      .toFile(path.join(__dirname, `../../assets/brawler/ui/shell/${name}.png`));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
