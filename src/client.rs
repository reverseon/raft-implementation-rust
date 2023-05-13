pub mod comms {
    tonic::include_proto!("raftrpc");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
