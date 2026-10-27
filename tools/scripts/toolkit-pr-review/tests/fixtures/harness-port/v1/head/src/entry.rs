/// Both deliveries must retain the caller's correlation tag.
/// Install the carrier once at ingress; downstream deliveries use the ambient value.
pub async fn dispatch(tag: String) -> (Option<String>, Option<String>) {
    crate::carrier::with_tag(tag, async {
        let left=crate::delivery::send_background().await;
        let right=crate::delivery::send_joined().await;
        (left,right)
    }).await
}
