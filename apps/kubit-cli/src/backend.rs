//! Which kernel the CLI runs on.
//!
//! Until ADR-0002 is accepted and its `kubit-kernel-<name>` crate lands, the CLI
//! uses the exact reference box kernel from `kubit-kernel-api` (feature
//! `testing`). It covers boxes and push/pull but not STEP, which then fails
//! with a clean `Unsupported` error.

use kubit_engine::KernelFactory;
use kubit_kernel_api::Kernel;
use kubit_kernel_api::testing::BoxKernel;

/// The kernel factory used by every CLI command.
pub fn factory() -> KernelFactory {
    Box::new(|| Box::new(BoxKernel::new()) as Box<dyn Kernel>)
}
