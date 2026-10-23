#import bevy_ui::ui_vertex_output::UiVertexOutput
@group(1) @binding(0) var<uniform> fill: vec4<f32>;
@group(1) @binding(1) var<uniform> ink: vec4<f32>;
@group(1) @binding(2) var<uniform> accent: vec4<f32>;
@group(1) @binding(3) var<uniform> geometry: vec4<f32>;
@group(1) @binding(4) var<uniform> flags: vec4<f32>;
@group(1) @binding(5) var<uniform> details: vec4<f32>;
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
    let extent=size-vec2<f32>(details.y+1.,details.z+1.);
    let d=edge(p,extent,1.);
    // Derivatives measure physical pixel coverage, including HiDPI/UI scaling.
    let aa=max(0.5*fwidth(d),0.0001);
    let mask=smoothstep(-aa,aa,d);
    let shadow_d=edge(p-vec2<f32>(details.y,details.z),extent,1.);
    let shadow_aa=max(0.5*fwidth(shadow_d),0.0001);
    let shadow=smoothstep(-shadow_aa,shadow_aa,shadow_d);
    let mid=extent.y*0.5;
    let left=geometry.z*(extent.y-p.y)+15.;
    let marker_x=p.x-left;
    let marker_d=min(marker_x,(12.-marker_x-abs(p.y-mid))/sqrt(2.));
    let marker_aa=max(0.5*fwidth(marker_d),0.0001);
    let marker=smoothstep(-marker_aa,marker_aa,marker_d);
    if flags.y > 0.5 && flags.x < 0.5 { return vec4<f32>(0.); }
    var color=mix(ink,fill,smoothstep(geometry.w-aa,geometry.w+aa,d));
    if flags.z>0.5 {
        color=mix(accent,ink,smoothstep(geometry.w+2.-aa,geometry.w+2.+aa,d));
        color=mix(color,fill,smoothstep(geometry.w+6.-aa,geometry.w+6.+aa,d));
    }
    if flags.x>0.5 {
        color=mix(ink,vec4<f32>(0.96,0.88,0.72,1.),smoothstep(geometry.w-aa,geometry.w+aa,d));
        color=mix(color,ink,smoothstep(geometry.w+2.-aa,geometry.w+2.+aa,d));
        color=mix(color,fill,smoothstep(geometry.w+4.-aa,geometry.w+4.+aa,d));
        color=mix(color,ink,marker);
    }
    let shadow_strength=select(0.35,0.58,flags.z>0.5);
    let outside=mix(ink.rgb,accent.rgb,select(0.,0.55,flags.z>0.5));
    let a=max(mask,shadow*shadow_strength);
    return vec4<f32>(mix(outside,color.rgb,mask),a);
}
