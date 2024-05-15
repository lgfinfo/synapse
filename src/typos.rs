use crate::pb::{Service, ServiceInstance};

impl From<ServiceInstance> for Service {
    fn from(value: ServiceInstance) -> Self {
        Self {
            id: value.id,
            name: value.name,
            address: value.address,
            port: value.port,
            active: value.status,
            schema: value.schema,
        }
    }
}
