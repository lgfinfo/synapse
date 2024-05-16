use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use futures::{Stream, StreamExt};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tokio::time;
use tonic::codegen::tokio_stream::wrappers::BroadcastStream;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Response, Status};
use tracing::{debug, error, warn};

use crate::pb::health_client::HealthClient;
use crate::pb::service_registry_server::ServiceRegistry;
use crate::pb::{
    HealthCheckRequest, OperationStatus, QueryRequest, QueryResponse, Service, ServiceInstance,
    ServiceInstanceIdentifier, ServiceStatus, SubscribeRequest,
};

pub type ServiceInstances = DashMap<ServiceId, ServiceInstance>;

pub type ServiceName = String;

/// pb hub structure
/// store the service information
pub type RegistryPool = Arc<DashMap<ServiceName, ServiceInstances>>;

pub type ServiceId = String;
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
        i: &mut ServiceInstance,
        status: ServiceStatus,
        pool: &RegistryPool,
        max_tries: i32,
    ) -> (bool, bool) {
        let mut need_notify = false;
        match pool.get(&i.name) {
            // service is unregistered
            None => (false, need_notify),
            Some(instances) => match instances.get_mut(&i.id) {
                // service is unregistered
                None => (false, need_notify),
                Some(mut instance) => {
                    // check the retries
                    let mut health = instance.health_check.clone().unwrap();
                    if instance.status == ServiceStatus::Down as i32
                        && status == ServiceStatus::Down
                    {
                        health.retries -= 1;
                    }

                    if instance.status != status as i32 {
                        if status == ServiceStatus::Down {
                            instance.status = ServiceStatus::Down as i32;
                            i.status.clone_from(&instance.status);
                        } else {
                            instance.status = ServiceStatus::Up as i32;
                            i.status.clone_from(&instance.status);
                            // reset the retries
                            health.retries = max_tries;
                        };
                        need_notify = true;
                    }
                    let retries = health.retries;
                    instance.health_check.clone_from(&Some(health));

                    (retries > 0, need_notify)
                }
            },
        }
    }

    pub fn health_check(&self, mut instance: ServiceInstance) {
        let addr = if cfg!(feature = "docker") && instance.address == "127.0.0.1" {
            format!(
                "{}://{}:{}",
                instance.scheme(),
                "host.docker.internal",
                instance.port
            )
        } else {
            format!(
                "{}://{}:{}",
                instance.scheme(),
                &instance.address,
                instance.port
            )
        };
        let pool = self.registry_pool.clone();
        let pub_sub = self.broadcaster.clone();
        tokio::spawn(async move {
            // open mod check
            let health = instance.health_check.as_ref().unwrap();
            let duration = Duration::from_secs(health.interval as u64);
            debug!("health check start: {:?}", &instance);

            let mut client = match Self::create_health_client(addr, health.timeout).await {
                Ok(client) => client,
                Err(err) => {
                    // service mod check configuration error
                    error!("create client failed: {:?}", err);
                    return;
                }
            };
            let req = HealthCheckRequest {
                service: health.endpoint.clone(),
            };
            let max_tries = health.retries;
            loop {
                time::sleep(duration).await;
                let result = client.check(req.clone()).await;

                let status = if result.is_ok() {
                    debug!("healt check success: {:?}", instance);
                    ServiceStatus::Up
                } else {
                    warn!("healt check failed: {:?}", instance);
                    ServiceStatus::Down
                };

                let (is_pass, need_notify) =
                    Self::modify_service_status(&mut instance, status, &pool, max_tries);
                if need_notify {
                    Self::broadcast_(instance.clone(), &pub_sub).await;
                }
                if !is_pass {
                    break;
                }
            }
        });
    }
    async fn create_health_client(
        addr: String,
        timeout: i32,
    ) -> Result<HealthClient<Channel>, Box<dyn std::error::Error>> {
        let endpoint = Endpoint::from_shared(addr)?
            .timeout(Duration::from_secs(timeout as u64))
            .connect_lazy();

        let client = HealthClient::new(endpoint);

        Ok(client)
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
                active: instance.status,
                scheme: instance.scheme,
            };
            if let Err(e) = chan.send(Ok(req)) {
                warn!("broadcast failed: {:?}", e);
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
        if let Some(existing_services) = self.registry_pool.get(&instance.name) {
            if let Some(existing_instance) = existing_services.get(&instance.id) {
                // Skip if the instance already registered and the same as the new one
                if instance == *existing_instance
                    && existing_instance.health_check.is_some()
                    && existing_instance.health_check.as_ref().unwrap().retries > 0
                {
                    return Ok(Response::new(OperationStatus {
                        success: true,
                        message: "service already registered".to_string(),
                    }));
                }
            }
        }
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

    // todo modify return obj to `Service`
    async fn query_services(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let name = request.into_inner().name;
        debug!("query services: {:?}", name);
        let instances = match self.registry_pool.get(&name) {
            None => Vec::new(),
            Some(ins) => ins
                .iter()
                .map(|x| Service::from(x.value().clone()))
                .collect(),
        };
        Ok(Response::new(QueryResponse {
            services: instances,
        }))
    }

    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Service, Status>> + Send>>;

    // todo could we return multiple streams for multiple subscribe?
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
