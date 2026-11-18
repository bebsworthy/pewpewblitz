// Inverted-hull fighter silhouette outline (BRL-0151).
//
// Drawn on a smoothed-normal copy of each fighter mesh with front faces culled. Each vertex is
// pushed outward along its screen-projected normal by a world-unit width projected at its depth,
// so the outline scales with the fighter under every map framing and is even all around the
// silhouette, while depth stays that of the body: world geometry in front of a fighter still hides it. The
// colour is flat (no lighting, no emissive), well below the bloom threshold, and picked by the
// mesh tag the relationship rim already writes: 0 untagged, 1 local, 2 ally, 3 enemy.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    skinning,
    view_transformations::position_world_to_clip,
}

struct FighterOutlineSettings {
    // Linear RGB outline colour per mesh tag; alpha unused.
    colors: array<vec4<f32>, 4>,
    // x: width in world units at the fighter's depth; y: world units the hull is pushed away from
    // the camera, so where body parts meet (neck, hands on the blaster) the shell stays behind
    // the touching surfaces instead of poking dark specks through.
    width: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> outline: FighterOutlineSettings;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
#ifdef SKINNED
    @location(2) joint_indices: vec4<u32>,
    @location(3) joint_weights: vec4<f32>,
#endif
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) tag: u32,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
#ifdef SKINNED
    // Skinned fighters (the Kenney characters) follow the body's joint matrices.
    let world_from_local = skinning::skin_model(
        vertex.joint_indices, vertex.joint_weights, vertex.instance_index
    );
    let world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
    let world_position =
        mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    let away_from_camera = normalize(world_position.xyz - view.world_position);
    var clip = position_world_to_clip(world_position.xyz + away_from_camera * outline.width.y);

    // Screen-space direction, world-scale width: the projected normal in pixels gives the
    // direction; `width × clip_from_view[1][1]` is the width in NDC height units times clip.w, so
    // the offset below is already in clip space at this vertex's depth.
    let viewport = view.viewport.zw;
    let clip_normal = (view.clip_from_world * vec4<f32>(world_normal, 0.0)).xy;
    let pixel_normal = clip_normal * viewport;
    let length_pixels = length(pixel_normal);
    if length_pixels > 1.0e-5 {
        let width_clip = outline.width.x * view.clip_from_view[1][1];
        let offset = (pixel_normal / length_pixels) * width_clip * vec2<f32>(viewport.y / viewport.x, 1.0);
        clip = vec4<f32>(clip.xy + offset, clip.zw);
    }

    var out: VertexOutput;
    out.position = clip;
    out.tag = min(mesh_functions::get_tag(vertex.instance_index), 3u);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(outline.colors[in.tag].rgb, 1.0);
}
