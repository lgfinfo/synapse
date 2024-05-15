pub mod client;
pub mod hub;

pub use crate::pb::{
    service_registry_client::ServiceRegistryClient, service_registry_server::ServiceRegistry,
    service_registry_server::ServiceRegistryServer, OperationStatus, QueryRequest, QueryResponse,
    Scheme, Service, ServiceInstance, ServiceInstanceIdentifier, ServiceStatus, ServingStatus,
    SubscribeRequest,
};
