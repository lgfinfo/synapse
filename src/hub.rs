use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use tracing::{debug, warn};

use crate::pb::health_client::HealthClient;
use crate::pb::report_status_client::ReportStatusClient;
use crate::pb::service_registry_server::ServiceRegistry;
use crate::pb::{
    HealthCheckRequest, OperationStatus, QueryRequest, QueryResponse, ReportRequest, ServiceActive,
    ServiceInstance, ServiceInstanceIdentifier, ServiceStatus,
};

pub type ServiceInstances = DashMap<ServiceId, ServiceInstance>;

pub type ServiceName = String;

/// pb hub structure
/// store the service information
pub type RegistryPool = Arc<DashMap<ServiceName, ServiceInstances>>;

pub type ServiceId = String;

/// pub-sub hub
pub type PubSubHub = DashMap<ServiceName, Vec<ServiceId>>;

pub type ReporterPool = Arc<DashMap<ServiceId, ReportStatusClient<Channel>>>;

/// register center
#[derive(Clone, Debug)]
pub struct Hub {
    /// register center
    pub registry_pool: RegistryPool,
    /// publish subscribe center
    pub pub_sub_hub: PubSubHub,
    pub reporter: ReporterPool,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            registry_pool: Arc::new(DashMap::new()),
            pub_sub_hub: DashMap::new(),
            reporter: Arc::new(DashMap::new()),
        }
    }

    pub fn register_pub_sub(&self, instance: &ServiceInstance) {
        for name in &instance.subscribed_services {
            self.pub_sub_hub
                .entry(name.clone())
                .or_default()
                .push(instance.id.clone());
        }
    }

    pub fn un_register_pub_sub(&self, instance: &ServiceInstance) {
        for name in &instance.subscribed_services {
            self.pub_sub_hub
                .entry(name.clone())
                .or_default()
                .push(instance.id.clone());
        }
    }

    fn modify_service_status(
        name: &ServiceName,
        id: &ServiceId,
        status: ServiceStatus,
        pool: &RegistryPool,
    ) -> bool {
        match pool.get(name) {
            // service is unregistered
            None => false,
            Some(instances) => match instances.get(id) {
                // service is unregistered
                None => false,
                Some(instance) => {
                    if instance.status != status as i32 {
                        instances.get_mut(id).unwrap().status = if status == ServiceStatus::Down {
                            ServiceStatus::Down as i32
                        } else {
                            ServiceStatus::Up as i32
                        };
                        // todo notify the subscribers
                    }
                    true
                }
            },
        }
    }
    pub fn register_reporter(&self, instance: &ServiceInstance) {
        let reporters = self.reporter.clone();
        let cloned_id = instance.id.clone();
        let addr = format!("http://{}:{}", &instance.address, &instance.port);
        let pool = self.registry_pool.clone();
        let check = instance.health_check.clone();
        let id = instance.id.clone();
        let name = instance.name.clone();
        tokio::spawn(async move {
            // todo use timeout instead of loop
            for i in 0..5 {
                if let Ok(channel) = Channel::from_shared(addr.clone()).unwrap().connect().await {
                    let client = ReportStatusClient::new(channel);
                    // todo return the service list which is subscribed by this service
                    reporters.insert(cloned_id, client);
                    break;
                }
                warn!("connect to service failed, retry: {}", i);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            // open health check
            if let Some(health) = check {
                let mut tick = tokio::time::interval(Duration::from_secs(health.interval as u64));
                let mut client = HealthClient::new(
                    Channel::from_shared(addr.clone())
                        .unwrap()
                        .connect()
                        .await
                        .unwrap(),
                );
                let req = HealthCheckRequest {
                    service: health.endpoint.clone(),
                };
                loop {
                    tick.tick().await;
                    let is_pass = match client.check(req.clone()).await {
                        Ok(_) => Self::modify_service_status(&name, &id, ServiceStatus::Up, &pool),
                        Err(e) => {
                            warn!("health check failed: {:?}", e);

                            Self::modify_service_status(&name, &id, ServiceStatus::Down, &pool)
                        }
                    };
                    if !is_pass {
                        break;
                    }
                }
            }
        });
    }

    pub async fn broadcast(&self, instance: ServiceInstance) {
        // broadcast to all subscribers
        if let Some(pub_sub_hub) = self.pub_sub_hub.get(&instance.name) {
            let pub_sub_hub = pub_sub_hub.value();
            let req = ReportRequest {
                id: instance.id.clone(),
                name: instance.name,
                address: instance.address,
                port: instance.port,
                active: ServiceActive::Active as i32,
            };
            for id2 in pub_sub_hub {
                if id2 != &instance.id {
                    if let Some(mut reporter) = self.reporter.get_mut(id2) {
                        let reporter = reporter.value_mut();
                        let res = reporter.report_status(req.clone()).await.unwrap();
                        debug!("report status: {:?}", res);
                    }
                }
            }
        }
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

/// implement grpc interfaces
#[async_trait]
impl ServiceRegistry for Hub {
    async fn register_service(
        &self,
        request: Request<ServiceInstance>,
    ) -> Result<Response<OperationStatus>, Status> {
        let instance = request.into_inner();
        debug!("register service: {:?}", &instance);

        // register to pub-sub hub
        if !instance.subscribed_services.is_empty() {
            self.register_pub_sub(&instance);

            // register reporter and open health check
            self.register_reporter(&instance);
        }

        // register to registry pool
        self.registry_pool
            .entry(instance.name.clone())
            .or_default()
            .insert(instance.id.clone(), instance.clone());

        // notify all subscribers
        self.broadcast(instance).await;

        Ok(Response::new(OperationStatus {
            success: true,
            message: "register service success".to_string(),
        }))
    }

    async fn unregister_service(
        &self,
        request: Request<ServiceInstanceIdentifier>,
    ) -> Result<Response<OperationStatus>, Status> {
        let identifier = request.into_inner();
        debug!("unregister service: {:?}", &identifier);
        let instances = self.registry_pool.get(&identifier.name);
        if instances.is_none() {
            return Ok(Response::new(OperationStatus {
                success: true,
                message: "service not found".to_string(),
            }));
        }
        let instances = instances.unwrap();
        if let Some((_, instance)) = instances.remove(&identifier.id) {
            // broadcast to all subscribers
            self.broadcast(instance).await;
        }

        // unregister from pub-sub hub
        if let Some(mut pub_sub_hub) = self.pub_sub_hub.get_mut(&identifier.name) {
            let pub_sub_hub = pub_sub_hub.value_mut();
            pub_sub_hub.retain(|id| id != &identifier.id);
        }

        // unregister from reporter pool
        self.reporter.remove(&identifier.id);

        Ok(Response::new(OperationStatus {
            success: true,
            message: "unregister service success".to_string(),
        }))
    }

    async fn query_services(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let name = request.into_inner().name;
        debug!("query services: {:?}", name);
        let instances = self
            .registry_pool
            .get(&name)
            .ok_or(Status::not_found("service not found"))?;
        Ok(Response::new(QueryResponse {
            services: instances.iter().map(|x| x.value().clone()).collect(),
        }))
    }
}
