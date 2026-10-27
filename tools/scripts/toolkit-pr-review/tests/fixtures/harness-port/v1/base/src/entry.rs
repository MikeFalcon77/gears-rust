/// Both deliveries must retain the caller's correlation tag.
pub async fn dispatch(tag: String) -> (Option<String>, Option<String>) {
    let left=crate::delivery::send_background(tag.clone()).await;
    let right=crate::delivery::send_joined(tag).await;
    (left,right)
}
