use tonic::transport::Server;
use tracing::Level;

use register_center::health_service::HealthService;
use register_center::hub;
use register_center::pb::health_server::HealthServer;
use register_center::pb::service_registry_server::ServiceRegistryServer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    let h = hub::Hub::new();
    let server = HealthServer::new(HealthService {});
    let registry_server = ServiceRegistryServer::new(h);
    Server::builder()
        .add_service(server)
        .add_service(registry_server)
        .serve("127.0.0.1:50051".parse().unwrap())
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use tonic::server::NamedService;
    use tonic::transport::Channel;

    use register_center::pb::health_check_response::ServingStatus;
    use register_center::pb::health_client::HealthClient;
    use register_center::pb::service_registry_client::ServiceRegistryClient;
    use register_center::pb::service_registry_server::ServiceRegistryServer;
    use register_center::pb::{HealthCheckRequest, QueryRequest, ServiceInstance};

    use crate::hub::Hub;

    #[tokio::test]
    async fn test_health_check() {
        let channel = Channel::from_static("http://127.0.0.1:50051")
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
        let channel = Channel::from_static("http://127.0.0.1:50051")
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
            health_endpoint: "".to_string(),
            r#type: 0,
            metadata: Default::default(),
            tags: vec![],
            subscribed_services: vec![],
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
            health_endpoint: "".to_string(),
            r#type: 0,
            metadata: Default::default(),
            tags: vec![],
            subscribed_services: vec![],
        };
        client.register_service(req).await.unwrap();
    }
}
