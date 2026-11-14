const fs = require('fs');
const sharp = require('sharp');
const path = require('path');
const base = path.resolve(__dirname, '../..');
async function write(name, w, h, body) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`;
  fs.writeFileSync(`${__dirname}/${name}.svg`, svg);
  await sharp(Buffer.from(svg)).png().toFile(`${base}/assets/brawler/ui/shell/${name}.png`);
}
(async () => {
  // BRL-0156: shell-bg gradient with two low-contrast tonal planes (BRL-0140 tokens).
  await write('game-selection-background', 1600, 900, '<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2e7bff"/><stop offset="1" stop-color="#1b3fa8"/></linearGradient></defs><path fill="url(#bg)" d="M0 0h1600v900H0z"/><path fill="#0e1638" fill-opacity=".14" d="M0 180 1100 900H0z"/><path fill="#ffffff" fill-opacity=".08" d="m0 900 1600-230v230z"/>');
  // Mode badges moved to icons/build_icons.py (BRL-1wms).
  for (const selected of [false, true]) for (const focus of [false, true]) {
    const p = 'M24 14 370 4Q382 4 380 18L362 490Q362 500 350 500L12 510Q3 510 4 496Z';
    await write(`game-card${selected ? '-selected' : ''}${focus ? '-focus' : ''}`, 400, 530,
      `<path d="${p}" transform="translate(8 10)" fill="#ff7909"/><path d="${p}" fill="${selected ? '#ff7909' : '#faebcf'}" stroke="#061a25" stroke-width="6"/>${focus ? `<g transform="translate(10 12) scale(.945 .95)"><path d="${p}" fill="none" stroke="#061a25" stroke-width="15"/><path d="${p}" fill="none" stroke="#faebcf" stroke-width="7"/></g><path d="m8 240 28 18-28 18z" fill="#faebcf" stroke="#061a25" stroke-width="5"/>` : ''}`);
  }
})();
