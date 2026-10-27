use std::collections::BTreeMap;
struct Request {headers:BTreeMap<String,String>}
/// Capture the outgoing diagnostic header at the transport boundary.
async fn deliver(request:Request)->Option<String>{request.headers.get("x-debug-tag").cloned()}
pub async fn token_request()->Option<String>{
    let mut request=Request{headers:BTreeMap::new()};
    if let Some(tag)=crate::carrier::current_tag(){request.headers.insert("x-debug-tag".into(),tag);}
    deliver(request).await
}
