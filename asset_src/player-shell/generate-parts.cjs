// Authored UI illustrations: catalog presentation families, not physical model attachments.
const fs=require('fs'); const path=require('path'); const sharp=require('sharp');
const shapes={
magazine:'<path d="M92 52h110l-14 115-28 28H80z" fill="#0c2936"/><path d="M92 52h110l-7 38H87z" fill="#ff7900"/><path d="m106 106-7 62m40-62-7 62m40-62-7 62" stroke="#64aba8" stroke-width="12"/>',
ammunition:'<path d="M67 93 87 45l20 48v96H67zM128 93l20-48 20 48v96h-40zM189 93l20-48 20 48v96h-40z" fill="#ff7900"/><path d="M67 108h40v62H67zM128 108h40v62h-40zM189 108h40v62h-40z" fill="#64aba8"/>',
barrel:'<path d="m51 114 160-50 31 27-10 63-161 39-29-29z" fill="#0c2936"/><path d="m85 104 127-36 18 18-134 43z" fill="#64aba8"/><path d="m53 112 33-9 12 77-28 11-28-27zM193 72l20-8 29 27-10 63-21 5 8-58z" fill="#ff7900"/><ellipse cx="64" cy="149" rx="18" ry="29" fill="#faecd2"/><ellipse cx="64" cy="149" rx="9" ry="18" fill="#0c2936"/>',
chamber:'<path d="m93 48 91 0 37 51v70l-36 31H92l-33-31V99z" fill="#0c2936"/><path d="M93 48h91l37 51H59z" fill="#ff7900"/><circle cx="141" cy="139" r="43" fill="#64aba8"/><circle cx="141" cy="139" r="23" fill="#faecd2"/>',
element:'<path d="m104 53 69 0 23 35v93l-26 23h-67l-26-23V88z" fill="#64aba8"/><path d="M105 37h66v30h-66zM99 185h77v22H99z" fill="#0c2936"/><path d="m145 84-38 59h28l-8 34 41-58h-30z" fill="#ff7900"/>'
};
(async()=>{for(const [name,shape] of Object.entries(shapes)){let svg=`<svg xmlns="http://www.w3.org/2000/svg" width="288" height="240" viewBox="0 0 288 240"><g stroke="#06202c" stroke-width="5" stroke-linejoin="round">${shape}</g></svg>`;fs.writeFileSync(path.join(__dirname,`part-${name}.svg`),svg);await sharp(Buffer.from(svg)).png().toFile(path.join(__dirname,`../../assets/brawler/ui/shell/part-${name}.png`));}})();
