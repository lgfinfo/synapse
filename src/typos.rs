use crate::pb::{Scheme, Service, ServiceInstance};
use crate::service::{QueryRequest, ServiceInstanceIdentifier, SubscribeRequest};
use std::fmt::Display;

impl From<ServiceInstance> for Service {
    fn from(value: ServiceInstance) -> Self {
        Self {
            id: value.id,
            name: value.name,
            address: value.address,
            port: value.port,
            active: value.status,
            scheme: value.scheme,
        }
    }
}

impl From<u8> for Scheme {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Https,
            _ => Self::Http,
        }
    }
}

impl From<&str> for Scheme {
    fn from(value: &str) -> Self {
        match value {
            "https" => Self::Https,
            _ => Self::Http,
        }
    }
}

impl Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http => write!(f, "http"),
            Self::Https => write!(f, "https"),
        }
    }
}

impl SubscribeRequest {
    pub fn new(service: String) -> Self {
        Self { service }
    }
}

impl QueryRequest {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl ServiceInstanceIdentifier {
    pub fn new(name: String, id: String) -> Self {
        Self { id, name }
    }
}
