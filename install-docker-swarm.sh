#!/bin/bash

# =============================
# CONFIGURATION
# =============================
HOSTS=("ocas" "ocas01" "ocas02" "ocas03" "ocas04" "ocas05")
USER="mwotila"
MANAGER="${HOSTS[0]}"

# =============================
# INSTALL ARM64-COMPATIBLE DOCKER
# =============================
install_docker() {
ssh -o StrictHostKeyChecking=no "$USER@$host" << EOF
set -e

echo "[*] Removing old Docker packages..."
for pkg in docker.io docker-doc docker-compose docker-compose-v2 podman-docker containerd runc; do 
    sudo apt-get remove -y \$pkg || true
done

echo "[*] Updating apt and installing dependencies..."
sudo apt-get update -y
sudo apt-get install -y ca-certificates curl gnupg

echo "[*] Adding Docker GPG key..."
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg \
    -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc

echo "[*] Adding Docker repository..."
echo "deb [arch=\$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] \
https://download.docker.com/linux/ubuntu \
\$(. /etc/os-release && echo \${UBUNTU_CODENAME:-\$VERSION_CODENAME}) stable" \
| sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

echo "[*] Updating package index..."
sudo apt-get update -y

echo "[*] Installing Docker CE..."
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

echo "[*] Enabling Docker service..."
sudo systemctl enable --now docker

echo "[*] Adding user to docker group..."
sudo usermod -aG docker $USER || true

echo "[✓] Docker CE installed successfully on $(hostname)."

EOF
}


create_directories() {
ssh -o StrictHostKeyChecking=no "$USER@$1" << EOF
set -e

# Create per-node directory for node_exporter
echo "[*] Creating /var/lib/node_exporter on $(hostname)..."
sudo mkdir -p /var/lib/node_exporter
sudo chmod 755 /var/lib/node_exporter

EOF
}


# =============================
# STEP 1: Install Docker on all nodes
# =============================
echo "=== Installing Docker on all hosts ==="
for host in "${HOSTS[@]}"; do
    echo ">>> $host"
    install_docker "$host"
done
echo ""

# =============================
# STEP 2: Detect local private IP
# =============================
MANAGER_IP=$(ssh -o StrictHostKeyChecking=no "$USER@$MANAGER" \
"hostname -I | tr ' ' '\n' | grep '^10\.' | head -n 1")

echo "Manager private IP detected: $MANAGER_IP"

# =============================
# STEP 3: Initialize Swarm
# =============================
echo "=== Initializing Docker Swarm on $MANAGER ==="
ssh -t -o StrictHostKeyChecking=no "$USER@$MANAGER" \
"docker swarm init --advertise-addr $MANAGER_IP"

WORKER_TOKEN=$(ssh -o StrictHostKeyChecking=no "$USER@$MANAGER" \
"docker swarm join-token -q worker")

echo "Worker token: $WORKER_TOKEN"

# =============================
# STEP 4: Join workers
# =============================
echo "=== Joining workers to swarm ==="

for host in "${HOSTS[@]:1}"; do
    echo ">>> Joining $host ..."
    ssh -t -o StrictHostKeyChecking=no "$USER@$host" \
    "docker swarm join --token $WORKER_TOKEN $MANAGER_IP:2377"
done

echo ""
echo "=============================="
echo " Docker Swarm Cluster Ready! "
echo "=============================="
echo "Manager: $MANAGER ($MANAGER_IP)"
echo "Workers:"
printf "%s\n" "${HOSTS[@]:1}"
echo "=============================="


echo "=== Creating required directories on all nodes ==="
for host in "${HOSTS[@]}"; do
    echo ">>> $host"
    create_directories "$host"
done


echo "=== Creating /var/mongodb on manager ==="
ssh -o StrictHostKeyChecking=no "$USER@$MANAGER" << EOF
sudo mkdir -p /var/mongodb
sudo chmod 755 /var/mongodb
EOF