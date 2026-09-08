"""Design-only SVG mockup, with overview geometry read from current authored maps."""
from pathlib import Path
import re, base64
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
root=Path(__file__).resolve().parents[3]
out=Path(__file__).resolve().parent
catalog=(root/'content/catalogs/map_assets.ron').read_text()
assets={}
for m in re.finditer(r'id: MapAssetId\((\d+)\), key: "([^"]+)".*?slot: (\w+),.*?footprint_cells: \(width: (\d+), height: (\d+)\)',catalog,re.S):
    i,key,slot,w,h=m.groups();assets[int(i)]=(key,slot,int(w),int(h))
def color(key):
    if 'grass' in key:return '#538557'
    if 'water' in key:return '#4cacc4'
    if 'wall' in key:return '#526975'
    if key=='ground':return '#b9c8b0'
    if 'sand' in key:return '#e1cf9f'
    if 'oil' in key:return '#d07745'
    if 'cover' in key or 'barrier' in key:return '#b08760'
    if 'chest' in key:return '#e3b550'
    if 'poison' in key:return '#b1cb64'
    if 'fire' in key or 'hazard' in key:return '#ec9461'
    if 'ice' in key:return '#91cfdb'
    return '#91b3ab'
def overview(key):
    src=(root/f'content/maps/builtin/{key}.ron').read_text()
    w,h=map(int,re.search(r'dimensions: \(width: (\d+), height: (\d+)\)',src).groups())
    default=int(re.search(r'default_surface_asset_id: MapAssetId\((\d+)\)',src)[1])
    cells=[]
    pattern=r'placement_id: MapPlacementId\((\d+)\), cell: \(x: (\d+), y: (\d+)\), asset_id: MapAssetId\((\d+)\), quarter_turns: (\d+), parameters: ([^\n]+)'
    found=list(re.finditer(pattern,src));assert len(found)==len(re.findall(r'(?<!first_)placement_id:',src))
    for m in found:
        ident,x,y,a,q,params=m.groups();cells.append((int(ident),int(x),int(y),int(a),int(q),params))
    pattern=r'first_placement_id: MapPlacementId\((\d+)\), min: \(x: (\d+), y: (\d+)\), width: (\d+), height: (\d+), asset_id: MapAssetId\((\d+)\), quarter_turns: (\d+)'
    rects=list(re.finditer(pattern,src));assert len(rects)==src.count('first_placement_id:')
    for m in rects:
        ident,x,y,rw,rh,a,q=map(int,m.groups())
        cells.extend((ident+dy*rw+dx,x+dx,y+dy,a,q,'') for dy in range(rh) for dx in range(rw))
    shapes=[f'<rect width="{w}" height="{h}" fill="{color(assets[default][0])}"/>']
    for ident,x,y,a,q,params in sorted(cells,key=lambda c:({'Surface':0,'Feature':1,'Decoration':2,'Marker':3}.get(assets[c[3]][1],3),c[0])):
        name,slot,aw,ah=assets[a]
        if q%2:aw,ah=ah,aw
        if 'PlayerSpawn' in params:
            team=int(re.search(r'team_slot: (\d+)',params)[1]);c='#38a9dd' if team==0 else '#e16a62'
            shapes.append(f'<circle cx="{x+.5}" cy="{h-y-.5}" r=".6" fill="{c}" stroke="#fff4dd" stroke-width=".18"/>');continue
        if slot=='Marker':continue
        stroke='' if slot=='Surface' else ' stroke="#29444b" stroke-opacity=".25" stroke-width=".06"'
        shapes.append(f'<rect x="{x}" y="{h-y-ah}" width="{aw}" height="{ah}" fill="{color(name)}"{stroke}/>')
    body=''.join(shapes)
    svg=f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">{body}</svg>'
    (out/f'{key}-overview.svg').write_text(svg)
    print(key, w,h,'expanded placements:',len(cells))
    return w,h,body
maps=[overview('feature-yard-wipeout'),overview('verdant-crossfire')]
ink='#061a25';cream='#faebcf';orange='#ff7909'
svg=[f'<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900"><rect width="1600" height="900" fill="{cream}"/><path d="M0 490 600 900H0Z" fill="#69aaa3"/><path d="m750 900 850-180v180Z" fill="{orange}"/>']
font=TTFont(root/'assets/brawler/ui/fonts/lilita-one-regular.ttf')
glyphs=font.getGlyphSet(); cmap=font.getBestCmap(); units=font['head'].unitsPerEm
def text(x,y,t,size=32,fill=ink):
    pen=SVGPathPen(glyphs); cursor=0
    for char in t:
        name=cmap.get(ord(char),'.notdef')
        glyphs[name].draw(TransformPen(pen,(1,0,0,1,cursor,0)))
        cursor+=font['hmtx'][name][0]
    svg.append(f'<path d="{pen.getCommands()}" fill="{fill}" transform="translate({x} {y}) scale({size/units} {-size/units})"/>')
text(60,90,'CHOOSE PRACTICE MAP',58)
text(63,138,'WIPEOUT · 3v3',26)
logo=base64.b64encode((root/'assets/brawler/ui/shell/logo.png').read_bytes()).decode()
svg.append(f'<image x="1330" y="35" width="210" height="125" href="data:image/png;base64,{logo}"/>')
for i,(w,h,body) in enumerate(maps):
    x=60+i*755;y=190;cw=725;ch=545
    svg.append(f'<rect x="{x+6}" y="{y+8}" width="{cw}" height="{ch}" rx="10" fill="{orange}"/><rect x="{x}" y="{y}" width="{cw}" height="{ch}" rx="10" fill="{orange if i else cream}" stroke="{ink}" stroke-width="5"/>')
    svg.append(f'<rect x="{x+18}" y="{y+18}" width="689" height="437" rx="5" fill="#dfe5d4"/>')
    scale=min(655/w,403/h);ox=x+(cw-w*scale)/2;oy=y+18+(437-h*scale)/2
    svg.append(f'<g transform="translate({ox} {oy}) scale({scale})">{body}</g>')
    text(x+30,y+510,['FEATURE YARD','VERDANT CROSSFIRE'][i],38)
    if i:svg.append(f'<rect x="{x+8}" y="{y+8}" width="{cw-16}" height="{ch-16}" rx="7" fill="none" stroke="{ink}" stroke-width="9"/><rect x="{x+8}" y="{y+8}" width="{cw-16}" height="{ch-16}" rx="7" fill="none" stroke="{cream}" stroke-width="4"/><path d="m{x-12} {y+250} 26 16-26 16Z" fill="{cream}" stroke="{ink}" stroke-width="4"/>')
svg.append(f'<path d="M1160 789h380l-13 75h-380Z" fill="{orange}" stroke="{ink}" stroke-width="5"/>')
text(1181,838,'START PRACTICE  ›',30)
svg.append('</svg>');(out/'practice-map-picker.svg').write_text(''.join(svg))
