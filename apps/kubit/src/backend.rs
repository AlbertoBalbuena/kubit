//! Which kernel the desktop app runs on.
//!
//! Same placeholder as `kubit-cli` (apps/kubit-cli/src/backend.rs): until ADR-0002
//! is accepted and its `kubit-kernel-<name>` crate lands, the app uses the exact
//! reference box kernel from `kubit-kernel-api` (feature `testing`). It covers boxes
//! and push/pull but not STEP, which then fails with a clean `Unsupported` error.

use kubit_engine::KernelFactory;
use kubit_kernel_api::Kernel;
use kubit_kernel_api::testing::BoxKernel;

/// The kernel factory used by the app.
pub fn factory() -> KernelFactory {
    Box::new(|| Box::new(BoxKernel::new()) as Box<dyn Kernel>)
}
