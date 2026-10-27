//! STL (binary and ASCII). Positions are written as `f32` millimetres.

use kubit_mesh::{FaceTag, TriMesh};
use std::fmt::Write as _;

/// Most triangles accepted when reading, to refuse hostile files (50 M ≈ 2.5 GB).
pub const MAX_TRIANGLES: u32 = 50_000_000;

/// STL errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StlError {
    /// The mesh has out-of-range indices or non-finite positions.
    #[error("invalid mesh: {0}")]
    InvalidMesh(String),
    /// Too many triangles for binary STL (u32 count) or for the reader limit.
    #[error("too many triangles: {0}")]
    TooLarge(u64),
    /// Truncated or malformed file.
    #[error("malformed STL: {0}")]
    Malformed(String),
}

fn normal(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 && len.is_finite() {
        // Narrowing to f32 is the STL format.
        #[allow(clippy::cast_possible_truncation)]
        n.map(|x| (x / len) as f32)
    } else {
        [0.0; 3]
    }
}

#[allow(clippy::cast_possible_truncation)]
fn f32s(p: [f64; 3]) -> [f32; 3] {
    p.map(|x| x as f32)
}

/// Binary STL. The 80-byte header names the units.
pub fn write_binary(mesh: &TriMesh) -> Result<Vec<u8>, StlError> {
    mesh.validate()
        .map_err(|d| StlError::InvalidMesh(format!("{d:?}")))?;
    let count = u32::try_from(mesh.triangles.len())
        .map_err(|_| StlError::TooLarge(mesh.triangles.len() as u64))?;
    let mut out = Vec::with_capacity(84 + 50 * mesh.triangles.len());
    let mut header = [b' '; 80];
    let text = b"Kubit binary STL, units: mm";
    for (slot, b) in header.iter_mut().zip(text) {
        *slot = *b;
    }
    out.extend_from_slice(&header);
    out.extend_from_slice(&count.to_le_bytes());
    for i in 0..mesh.triangles.len() {
        let Some([a, b, c]) = mesh.corners(i) else {
            return Err(StlError::InvalidMesh(format!("triangle {i}")));
        };
        for v in [normal(a, b, c), f32s(a), f32s(b), f32s(c)] {
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    Ok(out)
}

/// ASCII STL.
pub fn write_ascii(mesh: &TriMesh, name: &str) -> Result<String, StlError> {
    mesh.validate()
        .map_err(|d| StlError::InvalidMesh(format!("{d:?}")))?;
    let name: String = name
        .chars()
        .filter(|c| c.is_ascii_graphic())
        .take(64)
        .collect();
    let mut s = format!("solid {name}\n");
    for i in 0..mesh.triangles.len() {
        let Some([a, b, c]) = mesh.corners(i) else {
            return Err(StlError::InvalidMesh(format!("triangle {i}")));
        };
        let n = normal(a, b, c);
        let _ = writeln!(
            s,
            "  facet normal {:e} {:e} {:e}\n    outer loop",
            n[0], n[1], n[2]
        );
        for p in [a, b, c].map(f32s) {
            let _ = writeln!(s, "      vertex {:e} {:e} {:e}", p[0], p[1], p[2]);
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    let _ = writeln!(s, "endsolid {name}");
    Ok(s)
}

fn take<'a>(bytes: &'a [u8], at: &mut usize, n: usize) -> Result<&'a [u8], StlError> {
    let end = at
        .checked_add(n)
        .ok_or_else(|| StlError::Malformed("offset overflow".into()))?;
    let s = bytes
        .get(*at..end)
        .ok_or_else(|| StlError::Malformed("truncated".into()))?;
    *at = end;
    Ok(s)
}

fn f32_at(bytes: &[u8], at: &mut usize) -> Result<f32, StlError> {
    let s = take(bytes, at, 4)?;
    let arr: [u8; 4] = s
        .try_into()
        .map_err(|_| StlError::Malformed("truncated".into()))?;
    Ok(f32::from_le_bytes(arr))
}

/// Reads a binary STL into an unwelded mesh (3 vertices per triangle, tag 0).
pub fn read_binary(bytes: &[u8]) -> Result<TriMesh, StlError> {
    let mut at = 80;
    let raw = take(bytes, &mut at, 4)?;
    let count = u32::from_le_bytes(
        raw.try_into()
            .map_err(|_| StlError::Malformed("header".into()))?,
    );
    if count > MAX_TRIANGLES {
        return Err(StlError::TooLarge(u64::from(count)));
    }
    let expected = 84u64 + 50 * u64::from(count);
    if (bytes.len() as u64) < expected {
        return Err(StlError::Malformed(format!(
            "{count} triangles need {expected} bytes, file has {}",
            bytes.len()
        )));
    }
    let mut mesh = TriMesh::default();
    for _ in 0..count {
        let _normal = take(bytes, &mut at, 12)?;
        let base = u32::try_from(mesh.positions.len())
            .map_err(|_| StlError::TooLarge(u64::from(count)))?;
        for _ in 0..3 {
            let p = [
                f32_at(bytes, &mut at)?,
                f32_at(bytes, &mut at)?,
                f32_at(bytes, &mut at)?,
            ];
            mesh.positions.push(p.map(f64::from));
        }
        let _attr = take(bytes, &mut at, 2)?;
        mesh.triangles.push([base, base + 1, base + 2]);
        mesh.face_tags.push(FaceTag(0));
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn tetra() -> TriMesh {
        TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [0.0, 0.0, 10.0],
            ],
            triangles: vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [0, 3, 2]],
            face_tags: vec![FaceTag(0); 4],
        }
    }

    #[test]
    fn binary_round_trip_keeps_volume() {
        let m = tetra();
        let bytes = write_binary(&m).unwrap();
        assert_eq!(bytes.len(), 84 + 50 * 4);
        let back = read_binary(&bytes).unwrap();
        assert!((back.volume() - 1000.0 / 6.0).abs() < 1e-4);
        assert!(back.watertightness(1e-6).is_closed());
    }

    #[test]
    fn ascii_has_every_facet() {
        let s = write_ascii(&tetra(), "part 1").unwrap();
        assert!(s.starts_with("solid part1\n"));
        assert_eq!(s.matches("facet normal").count(), 4);
        assert!(s.ends_with("endsolid part1\n"));
    }

    #[test]
    fn invalid_meshes_are_rejected() {
        let mut m = tetra();
        m.positions[0][0] = f64::NAN;
        assert!(matches!(write_binary(&m), Err(StlError::InvalidMesh(_))));
    }

    proptest! {
        #[test]
        fn hostile_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = read_binary(&bytes);
        }
    }
}
