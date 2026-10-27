//! Which kernel the desktop app runs on: arris, the initial backend chosen in ADR-0002.
//! Swapping backends means changing only this factory.

use kubit_engine::KernelFactory;
use kubit_kernel_api::Kernel;
use kubit_kernel_arris::ArrisKernel;

/// The kernel factory used by the app.
pub fn factory() -> KernelFactory {
    Box::new(|| Box::new(ArrisKernel::new()) as Box<dyn Kernel>)
}
