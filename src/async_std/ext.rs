macro_rules! ext_func {
	($func: ident ($self: ident: $self_type: ty $(, $arg: ident: $type: ty)*) -> $return_type: ty) => {
		paste::paste! {
			#[xx_core::coroutines::async_trait_impl]
			async fn $func($self: $self_type $(, $arg: $type)*) -> $return_type {
				$self.[<async_ $func>]($($arg,)* xx_core::coroutines::runtime::get_context().await)
			}
		}
    }
}

pub(crate) use ext_func;
