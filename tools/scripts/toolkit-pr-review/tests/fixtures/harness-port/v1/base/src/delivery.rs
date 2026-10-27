pub async fn send_background(tag: String) -> Option<String> {
    tokio::spawn(async move { crate::carrier::with_tag(tag, async { crate::carrier::current_tag() }).await }).await.unwrap()
}
pub async fn send_joined(tag: String) -> Option<String> {
    tokio::spawn(async move { crate::carrier::with_tag(tag, async { crate::carrier::current_tag() }).await }).await.unwrap()
}
