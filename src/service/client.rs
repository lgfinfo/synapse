use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::Status;
use tracing::{debug, error};

use crate::service::hub::{ServiceId, ServiceName};
use crate::service::{
    QueryRequest, Scheme, Service, ServiceInstance, ServiceInstanceIdentifier,
    ServiceRegistryClient, SubscribeRequest,
};

#[derive(Debug, Clone)]
pub struct ServiceClient {
    connect_timeout: Option<Duration>,
    client: ServiceRegistryClient<Channel>,
}

pub struct ServiceClientBuilder {
    scheme: Scheme,
    server_host: Option<String>,
    server_port: Option<u16>,
    tls_config: Option<ClientTlsConfig>,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
}

impl Default for ServiceClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceClientBuilder {
    pub fn new() -> Self {
        Self {
            scheme: Scheme::Http,
            server_host: None,
            server_port: None,
            tls_config: None,
            timeout: None,
            connect_timeout: None,
        }
    }

    pub fn server_host(mut self, host: impl Into<String>) -> Self {
        self.server_host = Some(host.into());
        self
    }

    pub fn server_port(mut self, port: u16) -> Self {
        self.server_port = Some(port);
        self
    }

    pub fn tls_config(mut self, tls_config: ClientTlsConfig) -> Self {
        self.scheme = Scheme::Https;
        self.tls_config = Some(tls_config);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = Some(connect_timeout);
        self
    }

    pub async fn build(mut self) -> Result<ServiceClient, Box<dyn std::error::Error>> {
        let server_host = self.server_host.take().ok_or("Server host not set")?;
        let server_port = self.server_port.take().ok_or("Server port not set")?;

        let addr = format!("{}://{}:{}", self.scheme, server_host, server_port);
        debug!("Building endpoint, connecting to: {}", addr);

        let mut endpoint = Endpoint::from_shared(addr)?;

        if let Some(ref config) = self.tls_config {
            endpoint = endpoint.tls_config(config.clone())?;
        }

        if let Some(timeout) = self.timeout {
            endpoint = endpoint.timeout(timeout);
        }

        if let Some(connect_timeout) = self.connect_timeout {
            endpoint = endpoint.connect_timeout(connect_timeout);
        }

        let client = ServiceRegistryClient::connect(endpoint).await?;

        Ok(ServiceClient {
            connect_timeout: self.connect_timeout,
            client,
        })
    }
}

impl ServiceClient {
    pub fn builder() -> ServiceClientBuilder {
        ServiceClientBuilder::new()
    }

    pub async fn subscribe(
        &mut self,
        name: impl Into<String> + Send + 'static,
    ) -> Result<Receiver<Service>, Box<dyn std::error::Error>> {
        let name = name.into();
        debug!("Subscribe to service: {}", name);

        let (tx, rx) = tokio::sync::mpsc::channel(256);

        // spawn a task to subscribe to the service
        self.handle_service(tx, name).await;
        Ok(rx)
    }

    async fn handle_service(&self, tx: Sender<Service>, name: String) {
        let connect_timeout = self.connect_timeout.unwrap_or(Duration::from_secs(5));
        let mut client = self.client.clone();

        tokio::spawn(async move {
            let max_retries = 5;
            let mut retries = 0;
            loop {
                // subscribe to the service
                let req = SubscribeRequest::new(name.clone());
                let mut stream = match client.subscribe_to_service(req).await {
                    Ok(stream) => {
                        retries = 0;
                        stream.into_inner()
                    }
                    Err(e) => {
                        error!("Failed to subscribe to service: {}", e);
                        if retries < max_retries {
                            debug!("Attempting to reconnect... Attempt: {}", retries + 1);
                            time::sleep(connect_timeout).await;
                            retries += 1;
                            continue;
                        } else {
                            error!("Maximum retry attempts reached, giving up.");
                            // exit the task, and it will close the channel
                            return;
                        }
                    }
                };

                // 请求消息并处理错误
                while let Ok(Some(res)) = stream.message().await {
                    debug!("GOT = {:?}", res);
                    if let Err(e) = tx.send(res).await {
                        error!("Failed to send service to channel: {}", e);
                        break;
                    }
                }
            }
        });
    }
    pub async fn register(
        &mut self,
        instance: ServiceInstance,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Register service instance: {:?}", instance);

        let res = self.client.register_service(instance).await?;
        if !res.into_inner().success {
            return Err(Box::new(Status::new(
                tonic::Code::Internal,
                "Failed to register service instance",
            )));
        }
        Ok(())
    }

    pub async fn unregister(
        &mut self,
        name: ServiceName,
        id: ServiceId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!(
            "Unregister service instance -- name: {:?}; -- id: {:?}",
            name, id
        );

        let res = self
            .client
            .unregister_service(ServiceInstanceIdentifier::new(name, id))
            .await?;

        if !res.into_inner().success {
            return Err(Box::new(Status::new(
                tonic::Code::Internal,
                "Failed to unregister service instance",
            )));
        }
        Ok(())
    }

    pub async fn query_with_name(
        &mut self,
        name: impl AsRef<str>,
    ) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        debug!("Query service: {}", name.as_ref());

        let res = self
            .client
            .query_services(QueryRequest::new(name.as_ref().to_string()))
            .await?;
        Ok(res.into_inner().services)
    }
}

#[cfg(test)]
mod tests {
    use crate::service::client::ServiceClient;

    #[tokio::test]
    async fn test_new() {
        let mut client = ServiceClient::builder()
            .server_host("127.0.0.1")
            .server_port(8500)
            .build()
            .await
            .unwrap();
        let mut receiver = client.subscribe("test").await.unwrap();
        while let Some(client) = receiver.recv().await {
            println!("Received:{:?}", client);
        }
        println!("{:?}", client)
    }
}
