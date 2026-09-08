const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
(async () => {
  const outline = 'M30 12H676Q686 12 685 24L671 470Q670 480 660 480H19Q9 480 10 470L24 22Q24 12 30 12Z';
  for (const selected of [false,true]) for (const focus of [false,true]) {
    const name = `practice-card${selected?'-selected':''}${focus?'-focus':''}`;
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="700" height="500" viewBox="0 0 700 500"><path d="${outline}" transform="translate(8 9)" fill="#ff7909"/><path d="${outline}" fill="${selected?'#ff7909':'#faebcf'}" stroke="#061a25" stroke-width="6"/>${focus?`<g transform="translate(9 9) scale(.973 .964)"><path d="${outline}" fill="none" stroke="#061a25" stroke-width="15"/><path d="${outline}" fill="none" stroke="#faebcf" stroke-width="7"/></g><path d="m5 222 26 18-26 18z" fill="#faebcf" stroke="#061a25" stroke-width="4"/>`:''}</svg>`;
    fs.writeFileSync(path.join(__dirname, `${name}.svg`), svg);
    await sharp(Buffer.from(svg)).png().toFile(path.resolve(__dirname, `../../assets/brawler/ui/shell/${name}.png`));
  }
})();
