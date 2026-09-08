const fs=require('fs');const sharp=require('sharp');
const base=require('node:path').resolve(__dirname, '../..') + '/';
const ink='#061a25',cream='#faebcf',teal='#69aaa3',orange='#ff7909';
const panels=[['mode',600,320,cream,orange],['play',600,190,orange,orange],['practice',600,120,teal,orange],['build',400,110,teal,ink],['menu',200,80,orange,'#ad5709'],['identity',650,80,ink,'#c3b398']];
async function write(name,w,h,body){const svg=`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`;fs.writeFileSync(base+'asset_src/player-shell/'+name+'.svg',svg);await sharp(Buffer.from(svg)).resize(w*2,h*2).png().toFile(base+'assets/brawler/ui/shell/'+name+'.png');}
(async()=>{
for(const [name,w,h,fill,back] of panels){const s=name==='mode'?26:18;const p=`M${s+8} 7 L${w-16} 3 Q${w-7} 3 ${w-9} 13 L${w-s-5} ${h-20} Q${w-s-6} ${h-13} ${w-s-14} ${h-13} L10 ${h-9} Q3 ${h-9} 5 ${h-18} L${s} 15 Q${s+1} 7 ${s+8} 7 Z`;for (const focus of [false, true]) await write('panel-'+name+(focus?'-focus':''),w,h,`<defs><linearGradient id="f" x2=".25" y2="1"><stop stop-color="${fill}"/><stop offset="1" stop-color="${fill}" stop-opacity=".94"/></linearGradient></defs><path d="${p}" fill="${back}" transform="translate(6 8)"/><path d="${p}" fill="url(#f)" stroke="${ink}" stroke-width="6" stroke-linejoin="round"/><path d="M${s+9} 12 L${w-20} 8" stroke="white" stroke-opacity=".2" stroke-width="2"/>${focus?`<g transform="translate(9 7) scale(${(w-18)/w} ${(h-14)/h})"><path d="${p}" fill="none" stroke="${ink}" stroke-width="17" stroke-linejoin="round"/><path d="${p}" fill="none" stroke="${cream}" stroke-width="7" stroke-linejoin="round"/></g><path d="M${s+3} ${h/2-12} L${s+18} ${h/2} L${s+3} ${h/2+12}Z" fill="${cream}" stroke="${ink}" stroke-width="4" stroke-linejoin="round"/>`:""}`);}
const skull=`<path fill="white" d="M50 8 70 16 82 34 80 59 68 68 67 85 56 85 55 74 47 74 46 85 34 84 33 68 20 59 18 35 29 17Z"/><path fill="${orange}" d="m26 37 22 12-8 12-13-5zm48 0-22 12 8 12 13-5zM50 55l-7 12h14zM34 15l16 25 16-25-16 17z"/>`;
await write('wipeout',100,124,`<path fill="${ink}" opacity=".22" d="M8 7h88v85l-44 29L8 92Z"/><path fill="${orange}" stroke="#d45a09" stroke-width="2" d="M4 2h88v83l-44 28L4 85Z"/><g transform="translate(11 18) scale(.74)">${skull}</g>`);
await write('icon-skull',100,100,skull.replaceAll(orange,'#69aaa3'));
await write('icon-lightning',100,100,'<path fill="white" d="M85 3 22 56 48 61 26 97 84 43 57 38Z"/>');
await write('icon-crosshair',100,100,'<g fill="none" stroke="white" stroke-width="6"><circle cx="50" cy="50" r="31"/><circle cx="50" cy="50" r="12"/><path d="M50 5v25m0 40v25M5 50h25m40 0h25M18 18l10 10m44 44 10 10M18 82l10-10m44-44 10-10"/></g>');
await write('icon-menu',100,100,'<path d="M15 25h70M15 50h70M15 75h70" fill="none" stroke="white" stroke-width="10" stroke-linecap="round"/>');
await write('icon-player',100,100,'<g fill="white"><circle cx="50" cy="27" r="20"/><path d="M12 90V78c0-36 76-36 76 0v12Z"/></g>');
await write('icon-arena',100,100,'<path fill="none" stroke="white" stroke-width="10" stroke-linejoin="round" d="M12 83V34l20 10L50 14l18 30 20-10v49H62V67H38v16Z"/>');
await write('icon-chevron',100,100,'<path d="m34 14 32 36-32 36" fill="none" stroke="white" stroke-width="16" stroke-linejoin="round"/>');
await write('underline',400,20,`<path fill="${ink}" d="M4 8h362l-7-7 37 7-37 12 7-7H4Z"/>`);
})();
