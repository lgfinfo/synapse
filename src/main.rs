use clap::Parser;
use dotenv::dotenv;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::Level;

use synapse::health::HealthServer;
use synapse::health::HealthService;
use synapse::service::hub;
use synapse::service::ServiceRegistryServer;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(long, env = "SERVICE_ADDRESS", default_value = "127.0.0.1:8500")]
    address: SocketAddr,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    // 加载环境变量
    dotenv().ok();

    let cli = Cli::parse();

    let h = hub::Hub::new();
    let server = HealthServer::new(HealthService {});
    let registry_server = ServiceRegistryServer::new(h);
    Server::builder()
        .add_service(server)
        .add_service(registry_server)
        .serve(cli.address)
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use tonic::server::NamedService;
    use tonic::transport::Channel;

    use synapse::health::{HealthCheckRequest, HealthClient};
    use synapse::service::ServiceRegistryClient;
    use synapse::service::ServiceRegistryServer;
    use synapse::service::{QueryRequest, ServiceInstance};
    use synapse::service::{Scheme, ServingStatus};

    use crate::hub::Hub;

    #[tokio::test]
    async fn test_health_check() {
        let channel = Channel::from_static("http://127.0.0.1:8500")
            .connect()
            .await
            .unwrap();
        let mut client = HealthClient::new(channel);
        let request = tonic::Request::new(HealthCheckRequest {
            service: <ServiceRegistryServer<Hub> as NamedService>::NAME.to_string(),
        });
        let response = client.check(request).await.unwrap().into_inner();
        assert_eq!(response.status, ServingStatus::Serving as i32);
    }

    #[tokio::test]
    async fn test_register_service() {
        let channel = Channel::from_static("http://127.0.0.1:8500")
            .connect()
            .await
            .unwrap();
        let mut client = ServiceRegistryClient::new(channel);
        let req = ServiceInstance {
            id: "test12321321".to_string(),
            name: "test".to_string(),
            address: "127.0.0.1".to_string(),
            port: 8080,
            version: "".to_string(),
            metadata: Default::default(),
            tags: vec![],
            health_check: None,
            status: 0,
            scheme: Scheme::Http as i32,
        };
        client.register_service(req).await.unwrap();
        let response = client
            .query_services(QueryRequest {
                name: "test".to_string(),
            })
            .await
            .unwrap();

        println!("{:?}", response.into_inner());
    }

    #[tokio::test]
    async fn test_query() {
        let channel = Channel::from_static("http://127.0.0.1:8500")
            .connect()
            .await
            .unwrap();
        let mut client = ServiceRegistryClient::new(channel);
        let response = client
            .query_services(QueryRequest {
                name: "ws".to_string(),
            })
            .await
            .unwrap();

        println!("{:?}", response.into_inner());
    }

    #[tokio::test]
    async fn test_notify() {
        let channel = Channel::from_static("http://127.0.0.1:50051")
            .connect()
            .await
            .unwrap();
        let mut client = ServiceRegistryClient::new(channel);
        let req = ServiceInstance {
            id: "test12323".to_string(),
            name: "test2".to_string(),
            address: "127.0.0.1".to_string(),
            port: 50053,
            version: "".to_string(),
            metadata: Default::default(),
            tags: vec![],
            health_check: None,
            status: 0,
            scheme: Scheme::Http as i32,
        };
        client.register_service(req).await.unwrap();
    }
}
