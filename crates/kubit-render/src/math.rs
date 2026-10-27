//! Minimal f64 vector and matrix helpers for the camera (no GPU types).
//!
//! Matrices are column-major `[[f64; 4]; 4]` (`m[column][row]`), the layout WGSL's
//! `mat4x4<f32>` expects once converted to f32.

/// 3-vector.
pub type Vec3 = [f64; 3];
/// Column-major 4×4 matrix: `m[column][row]`.
pub type Mat4 = [[f64; 4]; 4];

pub(crate) fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub(crate) fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn length(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// Unit vector, or `None` for a (near) zero or non-finite vector.
pub(crate) fn normalize(a: Vec3) -> Option<Vec3> {
    let l = length(a);
    (l.is_finite() && l > 1e-300).then(|| scale(a, 1.0 / l))
}

/// `a * b`.
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    // Column c of the product is `a` applied to column c of `b`.
    b.map(|col| transform4(a, col))
}

/// `m * v` for a homogeneous vector.
pub fn transform4(m: &Mat4, v: [f64; 4]) -> [f64; 4] {
    let mut out = [0.0; 4];
    for (col, vk) in m.iter().zip(v) {
        for (o, c) in out.iter_mut().zip(col) {
            *o += c * vk;
        }
    }
    out
}

/// Projects a point: `m * [p, 1]` followed by the perspective divide.
pub fn project_point(m: &Mat4, p: Vec3) -> Option<Vec3> {
    let [x, y, z, w] = transform4(m, [p[0], p[1], p[2], 1.0]);
    (w.abs() > 1e-300).then(|| [x / w, y / w, z / w])
}

/// Inverse by 2×2 sub-determinants (Laplace expansion), or `None` if singular.
///
/// The formula is written for row-major `a[row][col]`; applied to our columns it
/// inverts the transpose, and `inv(Mᵀ) = inv(M)ᵀ`, so the output columns are right.
pub fn invert(m: &Mat4) -> Option<Mat4> {
    let [
        [a00, a01, a02, a03],
        [a10, a11, a12, a13],
        [a20, a21, a22, a23],
        [a30, a31, a32, a33],
    ] = *m;
    let s0 = a00 * a11 - a10 * a01;
    let s1 = a00 * a12 - a10 * a02;
    let s2 = a00 * a13 - a10 * a03;
    let s3 = a01 * a12 - a11 * a02;
    let s4 = a01 * a13 - a11 * a03;
    let s5 = a02 * a13 - a12 * a03;
    let c5 = a22 * a33 - a32 * a23;
    let c4 = a21 * a33 - a31 * a23;
    let c3 = a21 * a32 - a31 * a22;
    let c2 = a20 * a33 - a30 * a23;
    let c1 = a20 * a32 - a30 * a22;
    let c0 = a20 * a31 - a30 * a21;
    let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
    if !det.is_finite() || det.abs() < 1e-300 {
        return None;
    }
    let inv = [
        [
            a11 * c5 - a12 * c4 + a13 * c3,
            -a01 * c5 + a02 * c4 - a03 * c3,
            a31 * s5 - a32 * s4 + a33 * s3,
            -a21 * s5 + a22 * s4 - a23 * s3,
        ],
        [
            -a10 * c5 + a12 * c2 - a13 * c1,
            a00 * c5 - a02 * c2 + a03 * c1,
            -a30 * s5 + a32 * s2 - a33 * s1,
            a20 * s5 - a22 * s2 + a23 * s1,
        ],
        [
            a10 * c4 - a11 * c2 + a13 * c0,
            -a00 * c4 + a01 * c2 - a03 * c0,
            a30 * s4 - a31 * s2 + a33 * s0,
            -a20 * s4 + a21 * s2 - a23 * s0,
        ],
        [
            -a10 * c3 + a11 * c1 - a12 * c0,
            a00 * c3 - a01 * c1 + a02 * c0,
            -a30 * s3 + a31 * s1 - a32 * s0,
            a20 * s3 - a21 * s1 + a22 * s0,
        ],
    ];
    Some(inv.map(|col| col.map(|v| v / det)))
}

/// Converts to the f32 layout uploaded to the GPU.
pub fn to_f32(m: &Mat4) -> [[f32; 4]; 4] {
    m.map(|col| col.map(|v| v as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_of_product_is_identity() {
        let m: Mat4 = [
            [2.0, 0.5, 0.0, 0.0],
            [0.1, 3.0, 0.2, 0.0],
            [0.0, 0.3, 1.5, 0.0],
            [4.0, -2.0, 7.0, 1.0],
        ];
        let inv = invert(&m).unwrap();
        let id = mul(&m, &inv);
        for (c, col) in id.iter().enumerate() {
            for (r, v) in col.iter().enumerate() {
                let expected = if c == r { 1.0 } else { 0.0 };
                assert!((v - expected).abs() < 1e-12, "{c},{r}: {v}");
            }
        }
    }

    #[test]
    fn singular_matrix_has_no_inverse() {
        assert!(invert(&[[0.0; 4]; 4]).is_none());
    }
}
