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
  await write('game-selection-background', 1600, 900, '<path fill="#faebcf" d="M0 0h1600v900H0z"/><path fill="#69aaa3" d="M0 180 1100 900H0z"/><path fill="#ff7909" d="m0 900 1600-230v230z"/>');
  const icons = [
    ['hot-zone', '#318f8c', '<circle cx="100" cy="94" r="49" fill="none" stroke="#faebcf" stroke-width="15"/><circle cx="100" cy="94" r="20" fill="#faebcf"/>'],
    ['heist', '#061a25', '<rect x="49" y="47" width="102" height="99" rx="7" fill="none" stroke="#faebcf" stroke-width="12"/><circle cx="100" cy="96" r="24" fill="none" stroke="#faebcf" stroke-width="9"/><path d="M100 65v62M69 96h62" stroke="#faebcf" stroke-width="8"/>'],
    ['zombie', '#318f8c', '<path d="M58 61 81 40 127 43 115 57 143 65 148 124 130 148 72 148 52 119Z" fill="#faebcf"/><g fill="#318f8c"><circle cx="77" cy="91" r="13"/><circle cx="124" cy="91" r="13"/><path d="m80 123 12-9 10 11 11-11 12 10-12 15-11-9-10 9Z"/></g>']
  ];
  for (const [name, fill, icon] of icons) await write(`game-badge-${name}`, 200, 248, `<path d="M12 8h176v175l-88 54-88-54Z" fill="${fill}" stroke="#061a25" stroke-width="5"/>${icon}`);
  for (const selected of [false, true]) for (const focus of [false, true]) {
    const p = 'M24 14 370 4Q382 4 380 18L362 490Q362 500 350 500L12 510Q3 510 4 496Z';
    await write(`game-card${selected ? '-selected' : ''}${focus ? '-focus' : ''}`, 400, 530,
      `<path d="${p}" transform="translate(8 10)" fill="#ff7909"/><path d="${p}" fill="${selected ? '#ff7909' : '#faebcf'}" stroke="#061a25" stroke-width="6"/>${focus ? `<g transform="translate(10 12) scale(.945 .95)"><path d="${p}" fill="none" stroke="#061a25" stroke-width="15"/><path d="${p}" fill="none" stroke="#faebcf" stroke-width="7"/></g><path d="m8 240 28 18-28 18z" fill="#faebcf" stroke="#061a25" stroke-width="5"/>` : ''}`);
  }
})();
