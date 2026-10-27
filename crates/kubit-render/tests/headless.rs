//! Headless GPU tests. They pass with a note when no adapter exists (CI without a GPU).

// Helpers outside `#[test]` fns are not covered by clippy.toml's test allowances.
#![allow(clippy::unwrap_used, clippy::panic)]

use kubit_mesh::{FaceTag, TriMesh};
use kubit_render::{Camera, MeshId, RenderError, RenderStyle, Renderer, StandardView, Viewport};
use std::path::PathBuf;

const FRONT: FaceTag = FaceTag(3);
const TOP: FaceTag = FaceTag(2);

/// Cube [0, s]³, two triangles per face, vertices shared, tags 1..=6 (3 = front, −Y).
fn cube(s: f64) -> TriMesh {
    let faces: [(u64, [[f64; 3]; 4]); 6] = [
        (1, [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]]),
        (2, [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]]),
        (3, [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]]),
        (4, [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]]),
        (5, [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]]),
        (6, [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]]),
    ];
    let mut mesh = TriMesh::default();
    for (tag, quad) in faces {
        let base = mesh.positions.len() as u32;
        mesh.positions.extend(quad.iter().map(|p| p.map(|c| c * s)));
        mesh.triangles.push([base, base + 1, base + 2]);
        mesh.triangles.push([base, base + 2, base + 3]);
        mesh.face_tags.extend([FaceTag(tag), FaceTag(tag)]);
    }
    mesh
}

fn renderer() -> Option<Renderer> {
    match Renderer::headless() {
        Ok(r) => Some(r),
        Err(e @ (RenderError::NoAdapter(_) | RenderError::Device(_))) => {
            eprintln!("note: skipping GPU test, {e}");
            None
        }
        Err(e) => panic!("unexpected error: {e}"),
    }
}

fn framed(r: &Renderer, view: StandardView, size: Viewport) -> Camera {
    let mut camera = Camera::default();
    camera.set_view(view);
    camera.fit(&r.bounds().unwrap(), size.aspect());
    camera
}

#[test]
fn cube_renders_and_centre_pick_hits_front_face() {
    let Some(mut r) = renderer() else { return };
    r.upload(MeshId(1), &cube(20.0)).unwrap();
    let size = Viewport::new(320, 240);
    let camera = framed(&r, StandardView::Front, size);
    let style = RenderStyle::default();

    let image = r.render_to_image(&camera, size, &style).unwrap();
    assert_eq!(image.rgba.len(), 320 * 240 * 4);
    let centre = image.pixel(160, 120).unwrap();
    let corner = image.pixel(2, 2).unwrap();
    assert_ne!(
        centre, corner,
        "centre should show the cube, not the background"
    );

    let hit = r.pick(&camera, size, [160.0, 120.0]).unwrap();
    assert_eq!(hit, Some((MeshId(1), FRONT)));
    assert_eq!(r.pick(&camera, size, [2.0, 2.0]).unwrap(), None);
    assert_eq!(r.pick(&camera, size, [-5.0, 2.0]).unwrap(), None);
}

#[test]
fn pick_tracks_replaced_and_removed_meshes() {
    let Some(mut r) = renderer() else { return };
    r.upload(MeshId(7), &cube(10.0)).unwrap();
    r.upload(MeshId(7), &cube(30.0)).unwrap();
    let size = Viewport::new(64, 64);
    let camera = framed(&r, StandardView::Top, size);
    assert_eq!(
        r.pick(&camera, size, [32.0, 32.0]).unwrap(),
        Some((MeshId(7), TOP))
    );
    r.remove(MeshId(7));
    assert_eq!(r.pick(&camera, size, [32.0, 32.0]).unwrap(), None);
}

#[test]
fn highlighted_faces_change_colour() {
    let Some(mut r) = renderer() else { return };
    r.upload(MeshId(1), &cube(20.0)).unwrap();
    let size = Viewport::new(96, 96);
    let camera = framed(&r, StandardView::Front, size);
    let style = RenderStyle::default();
    let plain = r
        .render_to_image(&camera, size, &style)
        .unwrap()
        .pixel(48, 48);
    r.set_selected(&[(MeshId(1), FRONT)]);
    let selected = r
        .render_to_image(&camera, size, &style)
        .unwrap()
        .pixel(48, 48);
    r.set_selected(&[]);
    r.set_hovered(Some((MeshId(1), FRONT)));
    let hovered = r
        .render_to_image(&camera, size, &style)
        .unwrap()
        .pixel(48, 48);
    assert_ne!(plain, selected);
    assert_ne!(plain, hovered);
    assert_ne!(selected, hovered);
}

/// Writes `tests/snapshots/cube.png` when `KUBIT_UPDATE_SNAPSHOTS=1`, otherwise into
/// the target directory (GPUs differ, so the committed image is for eyes only).
#[test]
fn iso_snapshot_png() {
    let Some(mut r) = renderer() else { return };
    r.upload(MeshId(1), &cube(20.0)).unwrap();
    r.set_selected(&[(MeshId(1), TOP)]);
    r.set_hovered(Some((MeshId(1), FRONT)));
    let size = Viewport::new(640, 480);
    let camera = framed(&r, StandardView::Iso, size);
    let png = r
        .render_to_png(&camera, size, &RenderStyle::default())
        .unwrap();
    assert_eq!(&png[1..4], b"PNG");
    let path = if std::env::var_os("KUBIT_UPDATE_SNAPSHOTS").is_some() {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/cube.png")
    } else {
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cube.png")
    };
    std::fs::write(&path, png).unwrap();
    eprintln!("snapshot: {}", path.display());
}

#[test]
fn invalid_input_is_an_error_not_a_panic() {
    let Some(mut r) = renderer() else { return };
    let bad = TriMesh {
        positions: vec![[f64::NAN; 3]],
        triangles: vec![[0, 0, 0]],
        face_tags: vec![FaceTag(1)],
    };
    assert!(matches!(
        r.upload(MeshId(1), &bad),
        Err(RenderError::InvalidMesh(_))
    ));
    let empty = TriMesh::default();
    r.upload(MeshId(2), &empty).unwrap();
    let size = Viewport::new(16, 16);
    let camera = Camera::default();
    r.render_to_image(&camera, size, &RenderStyle::default())
        .unwrap();
    assert!(matches!(
        r.render_to_image(&camera, Viewport::new(1 << 20, 1), &RenderStyle::default()),
        Err(RenderError::ImageSize { .. })
    ));
    assert!(matches!(
        Renderer::new(r.device(), r.queue(), r.color_format(), 3),
        Err(RenderError::SampleCount(3))
    ));
}
