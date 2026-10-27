use std::future::Future;
tokio::task_local! { static TAG: String; }
pub async fn with_tag<F: Future>(tag: String, f: F) -> F::Output { TAG.scope(tag, f).await }
pub fn current_tag() -> Option<String> { TAG.try_with(Clone::clone).ok() }
#[cfg(test)] mod tests {
    use super::*;
    #[tokio::test] async fn tag_lifetime() {
        assert!(current_tag().is_none());
        assert_eq!(with_tag("abc".into(), async { current_tag() }).await.as_deref(), Some("abc"));
        assert!(current_tag().is_none());
    }
}
