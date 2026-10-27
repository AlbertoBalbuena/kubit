# Kernel spike — case geometry (contract for every adapter)

Every adapter builds **exactly** this geometry, so results are comparable. Constants live in
`harness/src/corpus.rs` (use them, do not retype numbers). Units: mm, radians. World axes; Z up.
"Box" = `BOX_A × BOX_B × BOX_C` = 100 × 60 × 40 with min corner at the origin, unless stated.

| Case | Geometry |
|---|---|
| 1a | Box − cylinder r=`HOLE_R` (10), axis +Z through (50, 30). The tool must overshoot: z ∈ [−1, 41]. |
| 1b | Box − cylinder r=10 at (50, 30), z ∈ [20, 41] (blind hole `BLIND_DEPTH` = 20 deep from the top). |
| 2a | Union of box [0,100]×[0,60]×[0,40] and box [100,200]×[0,60]×[0,40] (share the face x = 100). Oracle expects 6 faces if coplanar faces merge. |
| 2b | Union of box [0,100]×[0,60]×[0,40] and box [100,200]×[60,120]×[0,40] (touch only along the edge x=100, y=60). A clean error or a reported non-manifold result are both acceptable *results*; record what happens. |
| 3a | Union of box and cylinder r=10, axis +X through (0, 30, 50), x ∈ [0, 100]: lies tangent on the top face along a line. |
| 3b | Union of box and sphere r=10 centred (50, 30, 50): tangent to the top face at a point. |
| 4 | Plate [0,200]×[0,200]×[0,10] ∪ 100 cylinders r=`PIN_R` (4), z ∈ [`PIN_Z0`, `PIN_Z1`] = [5, 30], centres `corpus::pin_centres()`. Use the kernel's n-ary union if it has one, else fold one by one; say which in `notes`. |
| 5a | Box, fillet r=`FILLET_R` (5) on the edge along X at y=0, z=40 (top front edge). |
| 5b | Stadium prism: in the XY plane, two semicircles r=`STADIUM_R` (20) centred (0,0) and (`STADIUM_LS`=60, 0), joined by lines y=±20; extruded +Z by `STADIUM_H` (30). Fillet r=5 on the whole top boundary loop (4 tangent edges: select one edge if the kernel follows tangent chains, else all four). |
| 5c | Box, fillet r=5 on the three edges meeting at the corner (100, 60, 40): along X at (y=60,z=40), along Y at (x=100,z=40), along Z at (x=100,y=60). |
| 5d | Box, variable-radius fillet on the 5a edge, radius linear from `VAR_R0` (2) at x=0 to `VAR_R1` (6) at x=100. |
| 6 | Case 1a, then fillet r=`HOLE_FILLET_R` (2) on the hole's top rim (circle z=40, radius 10 about (50,30)). |
| 7a | Box, symmetric chamfer distance `CHAMFER_D` (5) on the 5a edge. |
| 7b | Box, chamfer on the 5a edge with distance 5 on the top face and angle `CHAMFER_ANGLE_DEG` (30°) — i.e. setbacks 5 (top) and 5·tan30° (front). N/E if unsupported. |
| 8a | Box shelled inward, thickness `SHELL_T` (2), top face (z=40) removed. |
| 8b | Cylinder r=`CYL8_R` (20), z ∈ [0, `CYL8_H`=40] about the Z axis, top rim filleted r=`CYL8_FILLET` (5), then shelled inward 2 mm with the bottom cap (z=0) removed. |
| 9a | Box with fillets r=5 on its four vertical edges; then push the top face (z=40) out by `PUSH_D` (+10) as a **direct face offset/move** (not a boolean with a new extrusion). N/E if the kernel has no face offset/move. |
| 9b | Same as 9a with −10. |
| 10a | Circle r=`HELIX_PROFILE_R` (2) in the XZ plane centred (`HELIX_R`=20, 0, 0) (meridian plane, contains the Z axis), swept along a helix about Z of radius 20, pitch `HELIX_PITCH` (10), `HELIX_TURNS` (3) turns, profile kept in the meridian plane (screw motion / fixed binormal Z). Caps on. |
| 10b | Ruled loft between circle r=`LOFT_R0` (20) at z=0 and circle r=`LOFT_R1` (10) at z=`LOFT_H` (30), both centred on the Z axis, capped (frustum). |
| 10c | Ruled (or smooth if only that exists) loft from rectangle `LOFT_RECT` 40×20 centred on the axis at z=0 to circle r=10 at z=30, capped. No volume oracle; validity only. |
| 11a | Revolve 360° about Z a half-disc r=`REV_SPHERE_R` (20) in the XZ plane: arc from (0,−20) through (20,0) to (0,20) in (x,z), closed by the segment on the axis. Result: sphere. |
| 11b | Revolve 360° about Z the triangle (0,0)→(`REV_CONE_R`=20, 0)→(0, `REV_CONE_H`=30) in (x,z). Result: cone with its apex on the axis. |
| 13a | Sphere r=`TESS_SPHERE_R` (20) at the origin (primitive or 11a construction), tessellated with the chord tolerance the runner passes (0.01). |
| 13b | Same, chord tolerance 0.1. |
| 13c | Case 1a tessellated at chord tolerance 0.01. |

## What `run` must return (`harness::Produced`)
- `op_seconds`: time **only the modelling operations** with `harness::timed` (no tessellation, mass properties, checks or STEP).
- `mesh`: tessellation at the `tol` argument, triangles wound CCW seen from outside, positions in mm. If the kernel cannot tessellate a B-rep, leave `None` and add a note.
- `topo`: V/E/F, loops (outer + inner per face) and shells as the kernel reports them; `backend_closed`/`backend_valid` from the kernel's own checks, naming the check in `backend_check`.
- `native_volume` / `native_area`: only from the kernel's own mass properties.
- `history`: what the API returned for the operation(s) of the case: `Native{..}` with counts, `Partial(..)`, or `None`. Do not reconstruct history yourself.
- `step`: the STEP text of the result when the kernel can write it, else `step_error` with the reason.
- `notes`: anything a reader of the ADR needs (e.g. "n-ary union", "chamfer is equal-setback only").

`import_step(text, tol)`: import every solid; mesh them into one `TriMesh`, sum native volumes; topology summed; note how many solids were read/refused.

`hostile(family, params)`: build the family's geometry from the raw params **without sanitising them** (the point is to see what the kernel does with NaN/∞/0/negatives). Families: `BoxMinusCylinder [a,b,c,r]` = box(0..a,0..b,0..c) − cylinder r at (a/2,b/2) overshooting z; `FilletEdge [a,b,c,r]` = fillet the top front edge; `PushPull [a,b,c,r,d]` = vertical fillets r then push the top face by d.

## Rules
- Missing operation → `harness::not_exposed(..)`; kernel error → `harness::failed(..)`. Never emulate a missing operation with other operations (e.g. no push/pull via boolean).
- A backend `unwrap`/`panic` inside the kernel is a **result**: do not wrap kernel calls in `catch_unwind` (the runner does it). Your own adapter code must not panic: no `unwrap`/`expect`.
- Read the dependency source for every signature you use; record them in `backends/<name>/NOTES.md`.
