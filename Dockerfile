# 第一阶段：构建环境
FROM debian:buster-slim AS builder

# Install a specific version of protoc

# 替换源以加速更新和安装过程
# 设置 protobuf 版本
ARG PROTOC_VERSION=26.1

# 安装 protoc 编译器和必要的包
RUN sed -i 's/deb.debian.org/mirrors.ustc.edu.cn/g' /etc/apt/sources.list && \
    apt-get update && \
    apt-get install -y curl gcc unzip && \
    curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v$PROTOC_VERSION/protoc-$PROTOC_VERSION-linux-x86_64.zip && \
    unzip -o protoc-$PROTOC_VERSION-linux-x86_64.zip -d /usr/local bin/protoc && \
    rm protoc-$PROTOC_VERSION-linux-x86_64.zip && \
    apt-get purge --autoremove -y unzip && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*
# 安装Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

# 配置环境变量以使用Rust命令
ENV PATH="/root/.cargo/bin:${PATH}"

# 设置工作目录
WORKDIR /usr/src/synapse

# 为了利用Docker的层缓存机制优化构建速度，首先只复制Cargo.toml和Cargo.lock
COPY Cargo.toml Cargo.lock ./

# 创建一个虚拟的源文件进行初始化构建，以缓存依赖项
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --features=docker

# 复制其余的源代码
COPY . .

# 设置环境变量
ENV PROTOC=/usr/local/bin/protoc

# 构建Rust应用
RUN cargo build --release --features=docker

# 第二阶段：运行环境
FROM debian:buster-slim

# 如果有运行时库依赖，记得在这里也要安装
# RUN sed -i 's/deb.debian.org/mirrors.ustc.edu.cn/g' /etc/apt/sources.list && \
#    apt-get update && \
#    apt-get install -y <需要的库> && \
#    apt-get clean && \
#    rm -rf /var/lib/apt/lists/*

# 从构建阶段复制编译好的可执行文件
COPY --from=builder /usr/src/synapse/target/release/synapse /usr/local/bin/synapse

# 运行时所需的环境变量
ENV SERVICE_ADDRESS=127.0.0.1:8500

# 设置工作目录（如果需要的话）
# WORKDIR /app

# 容器启动时运行的命令
CMD ["synapse"]
