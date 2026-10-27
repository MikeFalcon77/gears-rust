pub async fn send_background() -> Option<String> {
    tokio::spawn(async { crate::carrier::current_tag() }).await.unwrap()
}
pub async fn send_joined() -> Option<String> {
    let tag=crate::carrier::current_tag();
    tokio::spawn(async move {
        match tag { Some(tag) => crate::carrier::with_tag(tag, async { crate::carrier::current_tag() }).await, None => None }
    }).await.unwrap()
}
