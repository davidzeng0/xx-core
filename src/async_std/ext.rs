macro_rules! ext_func {
	($func: ident ($self: ident: $self_type: ty $(, $arg: ident: $type: ty)*) -> $return_type: ty) => {
		concat_idents::concat_idents!(func_name = async_, $func, {
			#[xx_core::coroutines::async_trait_impl]
			#[inline(always)]
			async fn $func($self: $self_type $(, $arg: $type)*) -> $return_type {
				Self::func_name($self, $($arg,)* xx_core::coroutines::runtime::get_context().await)
			}
		});
    }
}

pub(crate) use ext_func;
