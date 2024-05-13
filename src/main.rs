use register_center::health_service::HealthService;
use register_center::hub;
use register_center::pb::health_server::HealthServer;
use register_center::pb::service_registry_server::ServiceRegistryServer;
use tonic::transport::Server;

#[tokio::main]
async fn main() {
    let h = hub::Hub::new();
    let server = HealthServer::new(HealthService {});
    let registry_server = ServiceRegistryServer::new(h);
    Server::builder()
        .add_service(server)
        .add_service(registry_server)
        .serve("[::1]:50051".parse().unwrap())
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use register_center::pb::health_check_response::ServingStatus;
    use register_center::pb::health_client::HealthClient;
    use register_center::pb::service_registry_server::ServiceRegistryServer;
    use register_center::pb::HealthCheckRequest;
    use tonic::server::NamedService;
    use tonic::transport::Channel;

    use crate::hub::Hub;

    #[tokio::test]
    async fn test_health_check() {
        let channel = Channel::from_static("http://[::1]:50051")
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
}
