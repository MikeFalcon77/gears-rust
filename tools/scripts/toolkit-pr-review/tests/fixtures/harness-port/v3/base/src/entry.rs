/// Every token preflight request must carry the ingress diagnostic tag.
pub async fn handle(tag:String,mode:crate::proxy::AuthMode)->Vec<Option<String>> {
    tokio::spawn(async move { crate::carrier::with_tag(tag,crate::proxy::route(mode)).await }).await.unwrap()
}
