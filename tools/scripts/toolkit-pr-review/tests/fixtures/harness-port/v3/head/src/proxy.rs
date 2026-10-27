#[derive(Clone,Copy)] pub enum AuthMode { None, Token }
/// Primary forwarding uses the tunnel; token preflight uses the ordinary egress client.
pub async fn route(mode: AuthMode) -> Vec<Option<String>> {
    let mut observations=Vec::new();
    if matches!(mode,AuthMode::Token) { observations.push(crate::egress::token_request().await); }
    tunnel_forward().await;
    observations
}
async fn tunnel_forward() { tokio::task::yield_now().await; }
