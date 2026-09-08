"""Generate schematic Practice overviews from current authored map geometry."""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[2]
out=Path(__file__).resolve().parent / "map-overviews"
out.mkdir(exist_ok=True)
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
    placement_source=src.split('placements: [',1)[1].split('filled_rects:',1)[0]
    found=list(re.finditer(pattern,placement_source));assert len(found)==len(re.findall(r'(?<!first_)placement_id:',placement_source))
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
    for m in re.finditer(r'HotZoneCircle\(center_half_cell: \(x: (\d+), y: (\d+)\), radius_half_cells: (\d+)\)', src):
        x,y,r=map(int,m.groups())
        shapes.append(f'<circle cx="{x/2}" cy="{h-y/2}" r="{r/2}" fill="#ff9a31" fill-opacity=".3" stroke="#fff4dd" stroke-width=".2"/>')
    for m in re.finditer(r'HeistSafe\(team_slot: (\d+), origin_cell: \(x: (\d+), y: (\d+)\), width_cells: (\d+), height_cells: (\d+), quarter_turns: (\d+)', src):
        team,x,y,sw,sh,q=map(int,m.groups())
        if q%2: sw,sh=sh,sw
        c='#38a9dd' if team==0 else '#e16a62'
        shapes.append(f'<rect x="{x}" y="{h-y-sh}" width="{sw}" height="{sh}" fill="{c}" stroke="#fff4dd" stroke-width=".2"/>')
    body=''.join(shapes)
    svg=f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">{body}</svg>'
    (out/f'{key}-overview.svg').write_text(svg)
    print(key, w,h,'expanded placements:',len(cells))
    return w,h,body

for key in ["feature-yard-wipeout", "feature-yard-hot-zone", "feature-yard-heist", "verdant-crossfire", "switchback-basin", "powderline-vault", "zombie-yard"]: overview(key)
