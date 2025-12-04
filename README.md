# Yonga
A microservice placement strategy for low-resource settings using Docker Swarm.

---

## Table of Contents
1. [Overview](#1-overview)
2. [Prerequisites](#2-prerequisites)
3. [Set Up the Docker Swarm](#3-set-up-the-docker-swarm)
   - [Initialize the Swarm](#31-initialize-the-swarm-manager-node)
   - [Join Worker Nodes](#32-join-the-worker-nodes)
   - [Verify Cluster](#33-verify-the-cluster)
4. [Clone the Repository](#4-clone-the-repository)
5. [Deploy the Monitoring Stack](#5-deploy-the-monitoring-stack)
6. [Install Supporting Tools](#6-install-supporting-tools)
7. [Deploy the HotelReservation Application](#7-deploy-the-hotelreservation-application)
8. [Install Benchmarking Tools](#8-install-benchmarking-tools-wrk2--scripts)
9. [Network Performance Monitoring](#9-network-performance-monitoring)
10. [Install Rust and Cargo](#10-install-rust-and-cargo)
11. [Running Yonga](#11-running-yonga)

---

## 1. Overview
For this demonstration, we use **6 CloudLab nodes**:

- **1 manager node** – orchestrates the Docker Swarm, runs monitoring tools, and executes benchmarks (wrk2).
- **5 worker nodes** – run microservices and support distributed experiments.

The **hotelreservation** application from DeathStarBench acts as the test workload.

---

## 2. Prerequisites
Update all nodes:

```bash
sudo apt update && sudo apt upgrade -y
```

Install Docker manually or use the automatic setup script (For the experiments, we use Docker 5:23.0.6-1~ubuntu.22.04~jammy):

```bash
./install_docker_swarm.sh
```

Label the nodes appropriately in Docker Swarm: docker node update --label-add name=ocas05 9uo8i8n2889b0sgrauqj7xvml

```bash
docker node update --label-add name=ocasx [swarm-node-id]
......
docker node update --label-add name=ocasy [swarm-node-id]
```

---

## 3. Set Up the Docker Swarm

### 3.1 Initialize the Swarm (manager node)

```bash
docker swarm init --advertise-addr <MANAGER-IP>
```

### 3.2 Join the worker nodes

Run the join command on each worker node:

```bash
docker swarm join --token XXXXXX <MANAGER-IP>:2377
```

### 3.3 Verify the cluster

```bash
docker node ls
```

---

## 4. Clone the Repository

```bash
git clone https://github.com/crane-cloud/ocas.git
cd ocas
```

---

## 5. Deploy the Monitoring Stack

### 5.1 Prepare required directories

Manager:

```bash
sudo mkdir -p /var/mongodb
```

All nodes:

```bash
sudo mkdir -p /var/lib/node_exporter
```

### 5.2 Deploy monitoring

```bash
docker stack deploy --compose-file monitor.yaml monitor
```

Check:

```bash
docker stack ls
docker service ls
```

---

## 6. Install Supporting Tools

```bash
sudo apt install python3-pip iperf3 jq bc clang llvm-dev libclang-dev -y
pip3 install tcp-latency
```

---

## 7. Deploy the HotelReservation Application

```bash
docker stack deploy --compose-file docker-compose.yaml hotelreservation
```

---

## 8. Install Benchmarking Tools (wrk2 + scripts)

### 8.1 Dependencies

```bash
sudo apt install luarocks libssl-dev zlib-dev -y
sudo luarocks install luasocket
```

### 8.2 Build wrk2

```bash
cd /tmp
git clone https://github.com/giltene/wrk2.git
cd wrk2
make
cp wrk ~/ocas/
```

### 8.3 Fetch workload scripts

```bash
git clone https://github.com/delimitrou/DeathStarBench.git /tmp/DeathStarBench
cp -r /tmp/DeathStarBench/hotelReservation/wrk2/scripts ~/ocas/
```

### 8.4 Sample benchmark

```bash
./wrk -t 10 -c 100 -d 10   -s ./scripts/hotel-reservation/mixed-workload_type_1.lua   http://10.10.1.1:5000 -R 2000 -L
```

---

## 9. Network Performance Monitoring

Script:

```
/proj/cranecloud-PG0/ocas/scripts/yonga/perf.sh
```

Cron:

```
*/5 * * * * /bin/bash /proj/cranecloud-PG0/ocas/scripts/yonga/perf.sh X.X.X.X   >> /var/log/ocas-perf.log 2>&1 && sudo mv /tmp/metrics.txt /var/lib/node_exporter/yonga.prom
```

Prepare files:

```bash
sudo touch /var/lib/node_exporter/yonga.prom
sudo touch /var/log/ocas-perf.log
sudo chmod 666 /var/lib/node_exporter/yonga.prom
sudo chmod 666 /var/log/ocas-perf.log
```

Start iperf3 servers:

```bash
nohup iperf3 -s > /tmp/iperf3.log 2>&1 &
```

---

## 10. Install Rust and Cargo

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

---

## 11. Running Yonga

Run the monitor service:

```bash
cd ocas/yonga
cargo run --release --bin monitor   -- -c ../evaluation/config-dev.yaml
```

```bash
cd ocas/yonga
cargo run --release --bin ocas   -- -m ../docker-compose.yaml      -p yonga      -c ../evaluation/config-dev.yaml      -u http://127.0.0.1:31000      -s hotelreservation
```

Run the api service:

```bash
cd ocas/yonga
cargo run --release --bin api   -- - c../evaluation/config-dev.yaml      -p 31000



## Notes

Install docker-compose on all nodes
```bash
sudo apt install docker-compose -y
```

Change to the hotelReservation directory and rebuild the images 
```bash
cd /proj/cranecloud-PG0/ocas/hotelReservation
docker-compose build
```

Tag the images appropriately and push to your Docker Hub repository if needed.
```bash
docker tag hotelreservation_service_name your_dockerhub_username/hotelreservation_service_name:latest
docker push your_dockerhub_username/hotelreservation_service_name:latest
```

To build the jaeger-mongodb binary from source for your platform, use the following instructions (https://github.com/mongodb-labs/jaeger-mongodb):
```bash
git clone https://github.com/mongodb-labs/jaeger-mongodb.git
cd jaeger-mongodb
GOOS=linux GOARCH=arm64 CGO_ENABLED=0 go build ./cmd/jaeger-mongodb # Specify your target OS and architecture
```

You may need to change the permissions for writability of /var/lib/node_exporter/yonga.prom 
```bash
sudo chmod 666 /var/lib/node_exporter/yonga.prom
```

Run the script for multiple tests:
```bash
cd /proj/cranecloud-PG0/ocas/evaluation
for i in {1..10}; do      echo "=== Run $i ===";     ./generate_workload.sh random /proj/cranecloud-PG0/ocas/scripts/hotel-reservation/mixed-workload_type_1.lua;     sleep 10; done
```