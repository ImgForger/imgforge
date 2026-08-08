use imgforge::server;

#[tokio::main]
async fn main() -> Result<(), server::ServerError> {
    server::start().await
}
