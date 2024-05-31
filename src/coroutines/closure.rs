#![allow(clippy::module_name_repetitions)]

use std::marker::PhantomData;

use super::*;

#[asynchronous]
#[lang = "task_wrap"]
pub struct OpaqueTask<F, Output>(F, PhantomData<Output>);

#[asynchronous]
#[lang = "task_closure"]
pub struct OpaqueClosure<F, Output>(F, PhantomData<Output>);

#[asynchronous]
#[lang = "async_closure"]
pub struct OpaqueAsyncFn<F, const T: usize>(F);
