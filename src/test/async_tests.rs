use super::*;

#[async_fn]
async fn async_add(a: i32, b: i32) -> i32 {
	a + b
}

#[async_fn]
pub async fn test_add() {
	let result = async_add(12, 5).await;

	assert_eq!(result, 17);
}
