use synapse::health_service::HealthService;
use tonic::transport::{Channel, Server};
use tracing::Level;

use synapse::pb::health_server::HealthServer;
use synapse::pb::service_registry_client::ServiceRegistryClient;
use synapse::pb::{HealthCheck, ServiceInstance, SubscribeRequest};

// todo 客户端需要重连机制
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    let channel = Channel::from_static("http://127.0.0.1:50051")
        .connect()
        .await
        .unwrap();
    let mut client = ServiceRegistryClient::new(channel);
    // register self to center
    client
        .register_service(ServiceInstance {
            id: "1".to_string(),
            name: "ws".to_string(),
            address: "127.0.0.1".to_string(),
            port: 50001,
            version: "".to_string(),
            r#type: 0,
            metadata: Default::default(),
            tags: vec![],
            health_check: Some(HealthCheck {
                endpoint: "".to_string(),
                interval: 10,
                timeout: 10,
                retries: 10,
                schema: "https".to_string(),
            }),
            status: 0,
            schema: "https".to_string(),
        })
        .await
        .unwrap();
    tokio::spawn(async move {
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
    });
    let health = HealthServer::new(HealthService::new());
    Server::builder()
        .add_service(health)
        .serve("127.0.0.1:50001".parse().unwrap())
        .await
        .unwrap()
}
