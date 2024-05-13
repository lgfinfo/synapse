use register_center::pb::service_registry_client::ServiceRegistryClient;
use register_center::pb::SubscribeRequest;
use tonic::transport::Channel;

// todo 客户端需要重连机制
#[tokio::main]
async fn main() {
    let channel = Channel::from_static("http://127.0.0.1:50051")
        .connect()
        .await
        .unwrap();
    let mut client = ServiceRegistryClient::new(channel);
    let mut stream = client
        .subscribe(SubscribeRequest {
            service: "test".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    while let Some(message) = stream.message().await.unwrap() {
        println!("Received update from service 1: {:?}", message);
    }
}
