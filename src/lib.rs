pub mod async_std;
pub mod closure;
pub mod container;
pub mod coroutines;
pub mod error;
pub mod fiber;
pub mod future;
pub mod impls;
pub mod log;
pub mod macros;
pub mod opt;
pub mod os;
pub mod pointer;
pub mod runtime;
pub mod sync;
pub mod threadpool;

extern crate self as xx_core;

pub extern crate enumflags2;
pub extern crate lazy_static;
pub extern crate memchr;
pub extern crate num_traits;
pub extern crate paste;
pub extern crate static_assertions;
