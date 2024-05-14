use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use synapse::pb::health_server::{Health, HealthServer};
use synapse::pb::service_registry_client::ServiceRegistryClient;
use synapse::pb::{
    HealthCheck, HealthCheckRequest, HealthCheckResponse, ServiceInstance, SubscribeRequest,
};
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};
use tracing::debug;
use tracing::Level;

pub struct HealthCheckService;

#[async_trait]
impl Health for HealthCheckService {
    async fn check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        debug!("health check:{:?}", _request);
        Ok(Response::new(HealthCheckResponse { status: 1 }))
    }

    type WatchStream =
        Pin<Box<dyn Stream<Item = Result<HealthCheckResponse, Status>> + Send + 'static>>;

    async fn watch(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        todo!()
    }
}

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
            }),
            status: 0,
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
    let health = HealthServer::new(HealthCheckService);
    Server::builder()
        .add_service(health)
        .serve("127.0.0.1:50001".parse().unwrap())
        .await
        .unwrap()
}
