use aws_lambda_events::eventbridge::EventBridgeEvent;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::Value;
use tracing::info;

async fn handler(event: LambdaEvent<EventBridgeEvent<Value>>) -> Result<(), Error> {
    let payload = serde_json::to_string(&event.payload)?;
    info!(payload, "Elemental Inference event received");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().json().init();
    lambda_runtime::run(service_fn(handler)).await
}
