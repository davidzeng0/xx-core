use super::*;

channel_impl!(MCChannel);

impl<T> Clone for Receiver<T> {
	fn clone(&self) -> Self {
		Self::new(self.channel.clone())
	}
}
