pub mod async_std;
pub mod closure;
pub mod container;
pub mod coroutines;
pub mod error;
pub mod fiber;
pub mod impls;
pub mod log;
pub mod macros;
pub mod opt;
pub mod os;
pub mod pointer;
pub mod task;

extern crate self as xx_core;

#[cfg(any(test, feature = "test"))]
pub mod test;
