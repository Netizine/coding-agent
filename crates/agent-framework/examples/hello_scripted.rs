use agent_framework::{
    Message, ModelRequest, ResponseUpdate,
    runtime::{RunContext, collect, invoke},
};
use agent_test_support::ScriptedProvider;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ScriptedProvider::new([vec![
        Ok(ResponseUpdate::text("Hello ")),
        Ok(ResponseUpdate::text("from Rust.")),
    ]]);
    let request = ModelRequest {
        messages: vec![Message::user("Say hello")],
        ..ModelRequest::default()
    };
    let response = collect(invoke(&provider, &RunContext::default(), request).await?).await?;
    println!("{}", response.text());
    Ok(())
}
