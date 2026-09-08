// Run generate-map-overviews.py first; Sharp rasterizes these authored-geometry sources.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
const source = path.join(__dirname, 'map-overviews');
const output = path.resolve(__dirname, '../../assets/brawler/ui/shell');
(async () => {
  for (const name of fs.readdirSync(source).filter(name => name.endsWith('.svg'))) {
    await sharp(path.join(source, name)).resize(1200, 800, { fit: 'inside' }).png().toFile(path.join(output, name.replace('.svg', '.png')));
  }
})();
