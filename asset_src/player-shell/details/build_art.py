"""Code-authored shell geometry and pictograms. Run from repository root; requires Pillow.
No mockup pixels or third-party icon artwork are used. Output is supersampled PNG.
Frames are owned by the shared procedural shell surface renderer.
"""
from pathlib import Path
from PIL import Image, ImageDraw
import math
OUT=Path('assets/brawler/ui/details'); OUT.mkdir(parents=True,exist_ok=True)
S=3
INK='#061a25'; CREAM='#fff4dd'; TEAL='#078797'; CYAN='#27d2d2'; ORANGE='#ff830c'
def canvas(w,h):
 im=Image.new('RGBA',(w*S,h*S)); return im,ImageDraw.Draw(im)
def poly(d,points,fill): d.polygon([(x*S,y*S) for x,y in points],fill=fill)
def line(d,p,fill,width=5): d.line([(x*S,y*S) for x,y in p],fill=fill,width=width*S,joint='curve')
def ellipse(d,b,fill): d.ellipse(tuple(x*S for x in b),fill=fill)
def save(im,name): im.resize((im.width//S,im.height//S),Image.Resampling.LANCZOS).save(OUT/(name+'.png'))
# Small transparent navigation affordances.
for name in ['chevron','back','edit','trash','empty']:
 im,d=canvas(96,96)
 if name in ['chevron','back']:
  p=[(28,14),(41,14),(73,48),(41,82),(28,82),(58,48)]
  if name=='back': p=[(96-x,y) for x,y in p]
  poly(d,p,INK)
 elif name=='edit':
  poly(d,[(20,61),(59,22),(75,38),(36,77),(16,82)],INK); poly(d,[(64,17),(72,9),(87,24),(80,32)],INK)
 elif name=='trash':
  poly(d,[(25,29),(72,29),(67,84),(30,84)],INK);line(d,[(20,22),(77,22)],INK,7);line(d,[(38,20),(38,11),(58,11),(58,20)],INK,6)
  for x in [40,55]: line(d,[(x,40),(x,72)],CREAM,4)
 else:
  for x in range(12,84,18):
   line(d,[(x,7),(min(x+10,84),7)],TEAL,3);line(d,[(x,89),(min(x+10,84),89)],TEAL,3)
   line(d,[(7,x),(7,min(x+10,84))],TEAL,3);line(d,[(89,x),(89,min(x+10,84))],TEAL,3)
  line(d,[(30,48),(66,48)],'#7b9198',7);line(d,[(48,30),(48,66)],'#7b9198',7)
 save(im,name)
# All current semantic ability families; icons identify mechanics, names remain catalog owned.
names=['dash','sentry','cloak','scan','field','blast','cold','fire','poison','heal','blob','feather','shield','pulse','target','cycle']
for name in names:
 im,d=canvas(192,160)
 poly(d,[(44,7),(146,7),(186,80),(146,153),(44,153),(5,80)],TEAL)
 poly(d,[(47,14),(142,14),(178,80),(142,146),(47,146),(13,80)],INK)
 poly(d,[(51,21),(139,21),(170,80),(139,139),(51,139),(21,80)],TEAL if name=='dash' else INK)
 c=CYAN if name=='dash' else CREAM
 if name=='dash':
  ellipse(d,(105,32,129,56),c);line(d,[(108,64),(82,86),(111,106),(97,127)],c,12);line(d,[(98,75),(119,85),(145,72)],c,10);line(d,[(91,67),(66,59),(50,75)],c,10);line(d,[(81,91),(58,119),(36,122)],c,11)
  for y,x in [(44,56),(91,48),(110,32)]:poly(d,[(0,y),(x,y-3),(x,y+3)],CYAN)
 elif name=='feather':
  poly(d,[(55,115),(68,66),(111,34),(140,34),(132,74),(94,105)],c);line(d,[(48,127),(128,47)],INK,5)
 elif name=='shield':
  poly(d,[(96,32),(136,47),(129,96),(96,128),(62,96),(55,47)],c);poly(d,[(96,45),(123,56),(117,91),(96,112),(74,91),(68,56)],INK)
 elif name in ['pulse','heal']:
  if name=='pulse':line(d,[(40,82),(67,82),(80,54),(96,108),(113,67),(123,82),(151,82)],c,7)
  else:poly(d,[(84,42),(108,42),(108,68),(134,68),(134,92),(108,92),(108,118),(84,118),(84,92),(58,92),(58,68),(84,68)],c)
 elif name in ['target','scan']:
  ellipse(d,(55,39,137,121),c);ellipse(d,(63,47,129,113),INK);ellipse(d,(84,68,108,92),c)
  for p in [[(96,26),(96,52)],[(96,108),(96,134)],[(42,80),(68,80)],[(124,80),(150,80)]]:line(d,p,c,6)
 elif name=='cold':
  for a in [0,math.pi/3,2*math.pi/3]:line(d,[(96-42*math.cos(a),80-42*math.sin(a)),(96+42*math.cos(a),80+42*math.sin(a))],c,7)
 elif name=='fire':poly(d,[(96,29),(126,65),(139,99),(119,126),(74,126),(54,103),(60,78),(78,95),(78,57)],c)
 elif name=='poison':
  ellipse(d,(62,34,130,103),c);poly(d,[(78,95),(113,95),(113,118),(78,118)],c)
  ellipse(d,(72,58,90,78),INK);ellipse(d,(104,58,122,78),INK)
 elif name=='cycle':
  d.arc((53*S,37*S,139*S,123*S),35,310,fill=c,width=9*S);poly(d,[(130,31),(150,68),(110,64)],c)
 elif name=='sentry':
  poly(d,[(55,58),(141,58),(141,83),(109,83),(109,98),(78,98),(78,81),(55,81)],c);line(d,[(91,94),(64,126)],c,7);line(d,[(101,94),(126,126)],c,7)
 elif name=='cloak':
  poly(d,[(96,30),(125,52),(144,121),(48,121),(68,52)],c);ellipse(d,(78,54,113,92),INK)
 elif name=='field':
  d.arc((44*S,38*S,148*S,142*S),180,360,fill=c,width=7*S);line(d,[(43,92),(149,92)],c,7)
 elif name=='blob':
  for b in [(65,40,126,101),(44,81,99,130),(95,80,148,132)]:ellipse(d,b,c)
 else:poly(d,[(91,26),(105,60),(140,44),(125,76),(155,94),(118,100),(115,135),(90,112),(60,132),(64,99),(34,78),(72,69)],c)
 save(im,name)
