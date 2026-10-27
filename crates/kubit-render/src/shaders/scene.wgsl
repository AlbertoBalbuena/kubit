// Shaded faces, feature edges and the face-id pass. Layout mirrors `Globals` in gpu.rs.

const MAX_SELECTED: u32 = 16u;

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: unit direction towards the headlight (world space).
    light_dir: vec4<f32>,
    face_color: vec4<f32>,
    hover_color: vec4<f32>,
    selected_color: vec4<f32>,
    edge_color: vec4<f32>,
    // x: ambient, y: diffuse, z: hover mix (0..1).
    shading: vec4<f32>,
    // x: hovered face id (0 = none), y: number of selected ids.
    ids: vec4<u32>,
    selected: array<vec4<u32>, 4>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct FaceIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Mesh-local face index plus the mesh's first global id (per-instance).
    @location(2) local_face: u32,
    @location(3) id_base: u32,
};

struct FaceOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) @interpolate(flat) face_id: u32,
};

@vertex
fn vs_face(v: FaceIn) -> FaceOut {
    var out: FaceOut;
    out.clip = globals.view_proj * vec4<f32>(v.position, 1.0);
    out.normal = v.normal;
    out.face_id = v.id_base + v.local_face;
    return out;
}

fn is_selected(id: u32) -> bool {
    let count = min(globals.ids.y, MAX_SELECTED);
    for (var i = 0u; i < count; i = i + 1u) {
        if (globals.selected[i / 4u][i % 4u] == id) {
            return true;
        }
    }
    return false;
}

@fragment
fn fs_face(f: FaceOut) -> @location(0) vec4<f32> {
    var base = globals.face_color.rgb;
    if (is_selected(f.face_id)) {
        base = globals.selected_color.rgb;
    }
    if (globals.ids.x != 0u && globals.ids.x == f.face_id) {
        base = mix(base, globals.hover_color.rgb, globals.shading.z);
    }
    // Two-sided headlight Lambert: open shells and flipped normals still read.
    let lambert = abs(dot(normalize(f.normal), globals.light_dir.xyz));
    let light = globals.shading.x + globals.shading.y * lambert;
    return vec4<f32>(base * light, 1.0);
}

@fragment
fn fs_id(f: FaceOut) -> @location(0) u32 {
    return f.face_id;
}

@vertex
fn vs_edge(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return globals.view_proj * vec4<f32>(position, 1.0);
}

@fragment
fn fs_edge() -> @location(0) vec4<f32> {
    return globals.edge_color;
}

struct LineIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Ground grid and axes: per-vertex colour, alpha-blended.
@vertex
fn vs_line(v: LineIn) -> LineOut {
    var out: LineOut;
    out.clip = globals.view_proj * vec4<f32>(v.position, 1.0);
    out.color = v.color;
    return out;
}

@fragment
fn fs_line(f: LineOut) -> @location(0) vec4<f32> {
    return f.color;
}
