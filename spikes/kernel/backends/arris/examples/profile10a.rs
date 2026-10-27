//! Diagnostic: times each step of case 10a (helix sweep) separately.
use std::f64::consts::TAU;
use std::time::Instant;

use arris::check::{check, Level};
use arris::geom::{FrameLaw, Path, PathSegment, Profile, ProfileLoop};
use arris::math::{Axis, Frame, Point2, Point3, Vec3};
use arris::topo::Model;
use arris::Control;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tol: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0.01);
    let start = Point3::new(20.0, 0.0, 0.0);
    let profile = Profile {
        plane: Frame::new(start, -Vec3::y(), Vec3::x())?,
        outer: ProfileLoop::Circle { center: Point2::origin(), radius: 2.0 },
        holes: Vec::new(),
    };
    let path = Path {
        start,
        segments: vec![PathSegment::Helix { axis: Axis::z_at(Point3::origin()), pitch: 10.0, angle: TAU * 3.0 }],
    };
    let mut m = Model::default();
    let t = Instant::now();
    let (body, _) = arris::ops::sweep(&mut m, &profile, &path, FrameLaw::FixedBinormal(Vec3::z_axis()), &Control::NONE)?;
    eprintln!("sweep {:.2?}", t.elapsed());
    let t = Instant::now();
    let fast = check(&m, body, Level::Fast);
    eprintln!("check Fast {:.2?} ok={}", t.elapsed(), fast.is_ok());
    let t = Instant::now();
    let mp = arris::ops::measure::mass_properties(&m, body, &Control::NONE)?;
    eprintln!("mass {:.2?} V={}", t.elapsed(), mp.volume);
    let t = Instant::now();
    let mesh = arris::mesh::tessellate(&m, body, tol, &Control::NONE)?;
    eprintln!("tessellate({tol}) {:.2?} tris={}", t.elapsed(), mesh.triangles().len());
    let t = Instant::now();
    let text = arris::io::step::write(&m, &[body])?;
    eprintln!("step write {:.2?} {} bytes", t.elapsed(), text.len());
    let t = Instant::now();
    let full = check(&m, body, Level::Full);
    eprintln!("check Full {:.2?} ok={}", t.elapsed(), full.is_ok());
    Ok(())
}
