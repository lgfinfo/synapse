use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tonic::{Request, Response, Status};

use crate::pb::service_registry_server::ServiceRegistry;
use crate::pb::{
    OperationStatus, QueryRequest, QueryResponse, ServiceInstance, ServiceInstanceIdentifier,
};

pub type ServiceInstances = Arc<Vec<ServiceInstance>>;

pub type ServiceName = String;

/// pb hub structure
/// store the service information
pub type RegistryPool = DashMap<ServiceName, ServiceInstances>;

pub type ServiceId = String;

/// pub-sub hub
pub type PubSubHub = DashMap<ServiceName, Vec<Arc<ServiceId>>>;

/// register center
#[derive(Clone, Debug, Default)]
pub struct Hub {
    pub registry_pool: RegistryPool,
    pub pub_sub_hub: PubSubHub,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            registry_pool: DashMap::new(),
            pub_sub_hub: DashMap::new(),
        }
    }
}

/// implement grpc interfaces
#[async_trait]
impl ServiceRegistry for Hub {
    async fn register_service(
        &self,
        _request: Request<ServiceInstance>,
    ) -> Result<Response<OperationStatus>, Status> {
        todo!()
    }

    async fn unregister_service(
        &self,
        _request: Request<ServiceInstanceIdentifier>,
    ) -> Result<Response<OperationStatus>, Status> {
        todo!()
    }

    async fn query_services(
        &self,
        _request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        todo!()
    }
}
