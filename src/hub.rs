use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use futures::{Stream, StreamExt};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tonic::codegen::tokio_stream::wrappers::BroadcastStream;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use tracing::{debug, warn};

use crate::pb::health_client::HealthClient;
use crate::pb::service_registry_server::ServiceRegistry;
use crate::pb::{
    HealthCheckRequest, OperationStatus, QueryRequest, QueryResponse, Service, ServiceActive,
    ServiceInstance, ServiceInstanceIdentifier, ServiceStatus, SubscribeRequest,
};

pub type ServiceInstances = DashMap<ServiceId, ServiceInstance>;

pub type ServiceName = String;

/// pb hub structure
/// store the service information
pub type RegistryPool = Arc<DashMap<ServiceName, ServiceInstances>>;

pub type ServiceId = String;

/// pub-sub hub
pub type PubSubHub = DashMap<ServiceName, Vec<ServiceId>>;
pub type PubSub = DashMap<ServiceName, Sender<Result<Service, Status>>>;

/// register center
#[derive(Clone, Debug)]
pub struct Hub {
    /// register center
    registry_pool: RegistryPool,
    /// publish subscribe center
    broadcaster: PubSub,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            registry_pool: Arc::new(DashMap::new()),
            broadcaster: DashMap::new(),
        }
    }

    fn modify_service_status(
        name: &ServiceName,
        id: &ServiceId,
        status: ServiceStatus,
        pool: &RegistryPool,
    ) -> (bool, bool) {
        let mut need_notify = false;
        match pool.get(name) {
            // service is unregistered
            None => (false, need_notify),
            Some(instances) => match instances.get(id) {
                // service is unregistered
                None => (false, need_notify),
                Some(instance) => {
                    if instance.status != status as i32 {
                        instances.get_mut(id).unwrap().status = if status == ServiceStatus::Down {
                            ServiceStatus::Down as i32
                        } else {
                            ServiceStatus::Up as i32
                        };
                        need_notify = true;
                    }
                    (true, need_notify)
                }
            },
        }
    }
    pub fn health_check(&self, mut instance: ServiceInstance) {
        let addr = format!("http://{}:{}", &instance.address, &instance.port);
        let pool = self.registry_pool.clone();
        let pub_sub = self.broadcaster.clone();
        let check = instance.health_check.clone();
        // let id = instance.id.clone();
        // let name = instance.name.clone();
        tokio::spawn(async move {
            // open health check
            let health = check.unwrap();
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
                    Ok(_) => {
                        let (is_pass, need_notify) = Self::modify_service_status(
                            &instance.name,
                            &instance.id,
                            ServiceStatus::Up,
                            &pool,
                        );
                        if need_notify {
                            instance.status = ServiceStatus::Up as i32;
                            Self::broadcast_(instance.clone(), &pub_sub).await;
                        }
                        is_pass
                    }
                    Err(e) => {
                        warn!("health check failed: {:?}", e);

                        let (is_pass, need_notify) = Self::modify_service_status(
                            &instance.name,
                            &instance.id,
                            ServiceStatus::Down,
                            &pool,
                        );
                        if need_notify {
                            instance.status = ServiceStatus::Down as i32;
                            Self::broadcast_(instance.clone(), &pub_sub).await;
                        }
                        is_pass
                    }
                };
                if !is_pass {
                    break;
                }
            }
        });
    }

    pub async fn broadcast(&self, instance: ServiceInstance) {
        // broadcast to all subscribers
        Self::broadcast_(instance, &self.broadcaster).await;
    }

    async fn broadcast_(instance: ServiceInstance, broadcaster: &PubSub) {
        if let Some(chan) = broadcaster.get(&instance.name) {
            let chan = chan.value();
            let req = Service {
                id: instance.id.clone(),
                name: instance.name,
                address: instance.address,
                port: instance.port,
                active: ServiceActive::Active as i32,
            };
            chan.send(Ok(req)).unwrap();
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

        // register to registry pool
        self.registry_pool
            .entry(instance.name.clone())
            .or_default()
            .insert(instance.id.clone(), instance.clone());

        // check channel if exists
        self.broadcaster
            .entry(instance.name.clone())
            .or_insert(broadcast::channel(100).0);

        if instance.health_check.is_some() {
            self.health_check(instance.clone());
        }

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

    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Service, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let name = request.into_inner().service;
        debug!("subscribe: {:?}", name);
        let rx = match self.broadcaster.entry(name) {
            Entry::Occupied(res) => res.get().subscribe(),
            Entry::Vacant(entry) => {
                let (tx, rx) = broadcast::channel(100);
                entry.insert(tx);
                rx
            }
        };

        let stream = BroadcastStream::new(rx).map(|message_result| {
            message_result.unwrap_or_else(|recv_error| {
                // 如果是Err，则将BroadcastStream的错误转换成gRPC的错误
                Err(Status::internal(format!(
                    "Broadcast error: {:?}",
                    recv_error
                )))
            })
        });
        Ok(Response::new(Box::pin(stream)))
    }
}
