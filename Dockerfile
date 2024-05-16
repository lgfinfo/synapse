# 使用最新版本的 Rust 基础镜像
FROM rust:latest

# 设置工作目录为 /usr/src/my-app
WORKDIR /usr/src/synapse

# 安装 `protobuf-compiler` 和 `libprotobuf-dev`
RUN apt-get update && apt-get install -y protobuf-compiler libprotobuf-dev

# 复制 Rust 项目文件到工作目录
COPY . .

# 构建项目时，`build.rs` 脚本需要找到 `protoc`，所以我们在这里设置 `PROTOC` 环境变量
ENV PROTOC=/usr/bin/protoc

# 构建 Rust 应用的 release 版本
RUN cargo build --release --features=docker

# 设置默认的环境变量
ENV SERVICE_ADDRESS=127.0.0.1:8500

# 运行编译后的可执行文件
CMD ["./target/release/synapse"]
