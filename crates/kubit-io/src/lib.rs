//! File formats (L3). Phase 0: STL read/write. STEP text is produced and parsed
//! by the kernel backend (`kubit-kernel-api::Kernel::{export_step, import_step}`);
//! this crate only adds file-level helpers around it.
//!
//! Units: STL has none; Kubit writes millimetres and documents it in the header.

pub mod stl;
