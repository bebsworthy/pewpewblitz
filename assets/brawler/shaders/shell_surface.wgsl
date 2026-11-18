#import bevy_ui::ui_vertex_output::UiVertexOutput
// Gradient bottom; top is `fill_top`.
@group(1) @binding(0) var<uniform> fill: vec4<f32>;
@group(1) @binding(1) var<uniform> ink: vec4<f32>;
// Navigation focus color (frame, pointer, brackets or face highlight, by affordance).
@group(1) @binding(2) var<uniform> accent: vec4<f32>;
// x/y size, z slant, w border width.
@group(1) @binding(3) var<uniform> geometry: vec4<f32>;
// x focused, y quiet, z selected, w focus/selection affordance (BRL-fe7t review: 0 current
// frame, 1 bright frame, 2 reticle, 3 spotlight).
@group(1) @binding(4) var<uniform> flags: vec4<f32>;
// x corner cut, y/z bottom-lip offset.
@group(1) @binding(5) var<uniform> details: vec4<f32>;
@group(1) @binding(6) var<uniform> fill_top: vec4<f32>;
// Persistent selection rim.
@group(1) @binding(7) var<uniform> selection: vec4<f32>;
// x press amount (collapses the lip), y shine sweep 0..1 or negative when idle, z burst flash
// toward white (0..0.3; colors stay at or below 1.0), w focus pulse 0..1 (constant under reduced
// motion).
@group(1) @binding(8) var<uniform> motion: vec4<f32>;
fn edge(p: vec2<f32>, extent: vec2<f32>, inset: f32) -> f32 {
    let slope=geometry.z;
    let x=p.x+slope*p.y;
    let sides=min(min(p.y-inset,extent.y-inset-p.y),min((x-slope*extent.y-inset)/sqrt(1.+slope*slope),(extent.x-inset-x)/sqrt(1.+slope*slope)));
    // Opposite corner chamfers follow the slant; inset focus bands use the same contour.
    let cut=min(details.x,extent.y*0.16);
    let normal=sqrt(1.+(1.-slope)*(1.-slope));
    let top_right=(extent.x-x+p.y-cut)/normal-inset;
    let bottom_left=(x-slope*extent.y+extent.y-p.y-cut)/normal-inset;
    return min(sides,min(top_right,bottom_left));
}
// Second-nearest edge distance: small only near a corner of the silhouette (reticle brackets).
fn corner(p: vec2<f32>, extent: vec2<f32>) -> f32 {
    let slope=geometry.z;
    let x=p.x+slope*p.y;
    let side=sqrt(1.+slope*slope);
    let cut=min(details.x,extent.y*0.16);
    let normal=sqrt(1.+(1.-slope)*(1.-slope));
    let e=array<f32,6>(p.y,extent.y-p.y,(x-slope*extent.y)/side,(extent.x-x)/side,
        (extent.x-x+p.y-cut)/normal,(x-slope*extent.y+extent.y-p.y-cut)/normal);
    var a=1e9;
    var b=1e9;
    for (var i=0; i<6; i++) {
        let v=e[i];
        if v<a { b=a; a=v; } else if v<b { b=v; }
    }
    return b;
}
fn band(d: f32, start: f32, aa: f32) -> f32 {
    return smoothstep(start-aa,start+aa,d);
}
@fragment fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size=max(geometry.xy,vec2<f32>(1.));
    let p=in.uv*size;
    // The face keeps a fixed footprint; the ink lip below it collapses while pressed.
    let extent=size-vec2<f32>(details.y+1.,details.z+1.);
    let d=edge(p,extent,1.);
    // Derivatives measure physical pixel coverage, including HiDPI/UI scaling.
    let aa=max(0.5*fwidth(d),0.0001);
    let mask=smoothstep(-aa,aa,d);
    let lip_offset=details.yz*(1.-clamp(motion.x,0.,1.));
    let lip_d=edge(p-lip_offset,extent,1.);
    let lip_aa=max(0.5*fwidth(lip_d),0.0001);
    let lip=smoothstep(-lip_aa,lip_aa,lip_d);
    let mid=extent.y*0.5;
    let left=geometry.z*(extent.y-p.y)+15.;
    let marker_x=p.x-left;
    let marker_d=min(marker_x,(12.-marker_x-abs(p.y-mid))/sqrt(2.));
    let marker_aa=max(0.5*fwidth(marker_d),0.0001);
    let marker=smoothstep(-marker_aa,marker_aa,marker_d);
    if flags.y > 0.5 && flags.x < 0.5 { return vec4<f32>(0.); }
    let w=geometry.w;
    let variant=u32(flags.w+0.5);
    let focused=flags.x>0.5;
    let selected=flags.z>0.5;
    let pulse=clamp(motion.w,0.,1.);
    // Pressable face: vertical gradient plus a 2.5 px light band under the top border.
    var face=mix(fill_top,fill,clamp(p.y/max(extent.y,1.),0.,1.));
    if variant==3u && focused {
        // Spotlight: the focused face itself lights up toward the focus color.
        face=mix(face,accent,0.5+0.1*pulse);
    }
    let band_top=1.+w;
    let top_band=1.-smoothstep(band_top+2.,band_top+3.,p.y);
    face=mix(face,mix(face,vec4<f32>(1.),0.4),top_band);
    if motion.y >= 0. {
        // Diagonal shine sweep across the face (primary idle action or a burst).
        let travel=extent.x+extent.y;
        let along=p.x+0.6*(extent.y-p.y);
        let center=-0.2*travel+motion.y*1.4*travel;
        let shine=1.-smoothstep(4.,16.,abs(along-center));
        face=mix(face,vec4<f32>(1.),shine*0.38);
    }
    // Reward/confirmation burst flash (BRL-51zd).
    face=mix(face,vec4<f32>(1.),clamp(motion.z,0.,0.5));
    var color=vec4<f32>(0.);
    switch variant {
        case 1u: {
            // Bright frame: the outline turns white with a soft inner glow; selection is a gold
            // trim inside it. The surface also lifts (surface.rs).
            var inner=face;
            var edge_in=w;
            if focused {
                edge_in=w+1.5;
                // Lighten in hue (scale, not a mix toward white, which greys the navy roles),
                // then a narrow white glow just inside the frame.
                let lit=vec4<f32>(min(face.rgb*1.45,vec3<f32>(1.)),face.a);
                let glow=(1.-smoothstep(edge_in,edge_in+7.,d))*(0.16+0.14*pulse);
                inner=mix(lit,vec4<f32>(1.),glow);
            }
            if selected {
                inner=mix(selection,inner,band(d,edge_in+4.,aa));
            }
            color=mix(ink,inner,band(d,edge_in,aa));
            if focused {
                color=mix(ink,accent,band(d,1.5,aa));
                color=mix(color,inner,band(d,edge_in,aa));
            }
        }
        case 2u: {
            // Reticle: white corner brackets lock onto the silhouette; selection is a gold bar
            // along the bottom of the face.
            var inner=face;
            if selected {
                let bottom=extent.y-p.y;
                let bar=1.-smoothstep(w+6.-aa,w+6.+aa,bottom);
                let keyline=1.-smoothstep(w+8.-aa,w+8.+aa,bottom);
                inner=mix(inner,ink,keyline);
                inner=mix(inner,selection,bar);
            }
            color=mix(ink,inner,band(d,w,aa));
            if focused {
                let c=corner(p,extent);
                let c_aa=max(0.5*fwidth(c),0.0001);
                let reach=clamp(0.3*min(extent.x,extent.y),12.,26.)+6.*pulse;
                let arm=1.-smoothstep(reach-c_aa,reach+c_aa,c);
                let thick=w+3.;
                var bracket=mix(ink,accent,band(d,1.5,aa));
                bracket=mix(bracket,ink,band(d,thick,aa));
                color=mix(color,bracket,arm*(1.-band(d,thick+1.5,aa)));
            }
        }
        case 3u: {
            // Spotlight: the face lights up (above) under a white inner bevel; selection turns the
            // outline gold.
            var inner=face;
            if focused {
                inner=mix(vec4<f32>(1.),face,band(d,w+2.5,aa));
            }
            color=mix(ink,inner,band(d,w,aa));
            if selected {
                let rim=mix(ink,selection,band(d,1.5,aa));
                color=mix(rim,inner,band(d,w+1.,aa));
            }
        }
        default: {
            var inner=face;
            if selected {
                let rim=select(w+4.,w+9.,focused);
                inner=mix(selection,face,smoothstep(rim-aa,rim+aa,d));
            }
            color=mix(ink,inner,smoothstep(w-aa,w+aa,d));
            if focused {
                color=mix(ink,accent,smoothstep(w-aa,w+aa,d));
                color=mix(color,ink,smoothstep(w+4.-aa,w+4.+aa,d));
                color=mix(color,inner,smoothstep(w+6.-aa,w+6.+aa,d));
                color=mix(color,accent,marker);
            }
        }
    }
    let a=max(mask,lip);
    return vec4<f32>(mix(ink.rgb,color.rgb,mask),a);
}
