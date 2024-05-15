mod service;

pub use crate::pb::{
    health_client::HealthClient,
    health_server::{Health, HealthServer},
    HealthCheck, HealthCheckRequest, HealthCheckResponse,
};

pub use service::*;
