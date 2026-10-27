/// Every token preflight request must carry the ingress diagnostic tag.
pub async fn handle(tag:String,mode:crate::proxy::AuthMode)->Vec<Option<String>> {
    crate::carrier::with_tag(tag,async { tokio::spawn(crate::proxy::route(mode)).await.unwrap() }).await
}
