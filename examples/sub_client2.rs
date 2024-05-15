use tonic::transport::{Channel, Server};
use tracing::Level;
use tracing::{debug, info};

use synapse::health::HealthServer;
use synapse::health::{HealthCheck, HealthService};
use synapse::service::ServiceRegistryClient;
use synapse::service::{Scheme, ServiceInstance, SubscribeRequest};

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
    let result = client
        .register_service(ServiceInstance {
            id: "12".to_string(),
            name: "test".to_string(),
            address: "127.0.0.1".to_string(),
            port: 50002,
            version: "".to_string(),
            metadata: Default::default(),
            tags: vec![],
            health_check: Some(HealthCheck {
                endpoint: "".to_string(),
                interval: 10,
                timeout: 10,
                retries: 10,
                scheme: Scheme::Http as i32,
                tls_domain: None,
            }),
            status: 0,
            scheme: Scheme::Http as i32,
        })
        .await
        .unwrap();
    info!("register result: {:?}", result);
    tokio::spawn(async move {
        let mut stream = client
            .subscribe(SubscribeRequest {
                service: "ws".to_string(),
            })
            .await
            .unwrap()
            .into_inner();
        while let Some(message) = stream.message().await.unwrap() {
            debug!("Received update from service: {:?}", message);
        }
    });
    let health = HealthServer::new(HealthService::new());
    Server::builder()
        .add_service(health)
        .serve("127.0.0.1:50002".parse().unwrap())
        .await
        .unwrap()
}
