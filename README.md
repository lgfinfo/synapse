# Synapse

Welcome to **synapse**, the dynamic service discovery and registry solution designed for microservices architectures that need robust and real-time communication. Built with a focus on gRPC for efficient service-to-service communication, it also lays the foundation for future HTTP support, making it a versatile choice for diverse microservice ecosystems.

## Features

- **gRPC Support**: Utilizes gRPC for high-performance, secure communication between services, supporting advanced patterns like streaming and the robust Pub/Sub model.
- **Dynamic Service Discovery**: Automatically detects service instances, making it easier to scale up or down without manual intervention.
- **Health Checks**: Implements thorough health checks, ensuring that communications are only directed to healthy instances, hence maintaining the reliability of your system.
- **Future-proof**: Designed with an eye on the future, anticipating full support for HTTP-based services to cater to a wider range of service communication needs.

## Getting Started

To get started with **synapse**, ensure that you have the latest version of Rust installed. This project uses `cargo` for dependency management and builds.

### Installation

1. Clone the repository:

bash

```bash
git clone https://github.com/Xu-Mj/synapse.git
cd synapse
```

1. Build the project:

shell

```shell
cargo build --release
```

1. Run **synapse**:

shell

```shell
./target/release/synapse
```
