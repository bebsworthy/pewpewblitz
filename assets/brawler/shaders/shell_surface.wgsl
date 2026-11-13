#import bevy_ui::ui_vertex_output::UiVertexOutput
// Gradient bottom; top is `fill_top`.
@group(1) @binding(0) var<uniform> fill: vec4<f32>;
@group(1) @binding(1) var<uniform> ink: vec4<f32>;
// Navigation focus frame and pointer.
@group(1) @binding(2) var<uniform> accent: vec4<f32>;
// x/y size, z slant, w border width.
@group(1) @binding(3) var<uniform> geometry: vec4<f32>;
// x focused, y quiet, z selected.
@group(1) @binding(4) var<uniform> flags: vec4<f32>;
// x corner cut, y/z bottom-lip offset.
@group(1) @binding(5) var<uniform> details: vec4<f32>;
@group(1) @binding(6) var<uniform> fill_top: vec4<f32>;
// Persistent selection rim.
@group(1) @binding(7) var<uniform> selection: vec4<f32>;
// x press amount (collapses the lip), y shine sweep 0..1 or negative when idle.
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
    // Pressable face: vertical gradient plus a 2.5 px light band under the top border.
    var face=mix(fill_top,fill,clamp(p.y/max(extent.y,1.),0.,1.));
    let band_top=1.+w;
    let band=1.-smoothstep(band_top+2.,band_top+3.,p.y);
    face=mix(face,mix(face,vec4<f32>(1.),0.4),band);
    if motion.y >= 0. {
        // Diagonal shine sweep across the face for the primary idle action.
        let travel=extent.x+extent.y;
        let along=p.x+0.6*(extent.y-p.y);
        let center=-0.2*travel+motion.y*1.4*travel;
        let shine=1.-smoothstep(4.,16.,abs(along-center));
        face=mix(face,vec4<f32>(1.),shine*0.38);
    }
    var inner=face;
    if flags.z>0.5 {
        let rim=select(w+4.,w+9.,flags.x>0.5);
        inner=mix(selection,face,smoothstep(rim-aa,rim+aa,d));
    }
    var color=mix(ink,inner,smoothstep(w-aa,w+aa,d));
    if flags.x>0.5 {
        color=mix(ink,accent,smoothstep(w-aa,w+aa,d));
        color=mix(color,ink,smoothstep(w+4.-aa,w+4.+aa,d));
        color=mix(color,inner,smoothstep(w+6.-aa,w+6.+aa,d));
        color=mix(color,accent,marker);
    }
    let a=max(mask,lip);
    return vec4<f32>(mix(ink.rgb,color.rgb,mask),a);
}
