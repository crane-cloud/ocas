## Deploy the Monitor Infrastructure
```docker stack deploy --compose-file monitor.yaml monitor```


## Run OCAS
```./target/debug/ocas -m docker-compose.yaml -p yonga -c ../evaluation/config.yaml -u http://127.0.0.1:30000 -s hotelreservation```


## Start the API
```./target/debug/api -c ../evaluation/config.yaml -p 30000```

## Start the Monitor
```./target/debug/monitor -c ../evaluation/config.yaml```


## Sample NR/NE Results
```Resource for node cr-jhb: Resource { cpu: 15.756490499324578, memory: 46487.37109375, disk: 1333.6257209777832, network: 346252459.4786167 }
Network for node cr-jhb: Network { available: 1.0, bandwidth: 3.2475, latency: 97.93633843946736, packet_loss: 75.06835269993165 }

Resource for node cr-dar: Resource { cpu: 1.8242245803131372, memory: 12954.58203125, disk: 24.274490356445313, network: 117.2738800048828 }
Network for node cr-dar: Network { available: 0.75, bandwidth: 0.75, latency: 441.0004549426958, packet_loss: 100.0 }

Resource for node cr-kla: Resource { cpu: 1.7751798894737387, memory: 15164.70703125, disk: 210.36806106567383, network: 21.009642601013184 }
Network for node cr-kla: Network { available: 0.25, bandwidth: 0.25, latency: 794.7316314384807, packet_loss: 100.0 }

Resource for node cr-lsk: Resource { cpu: 1.788709869715509, memory: 13275.046875, disk: 161.1087188720703, network: 81.66149806976318 }
Network for node cr-lsk: Network { available: 0.5, bandwidth: 0.5, latency: 715.1329572079703, packet_loss: 100.0 }

Resource for node cr-bun: Resource { cpu: 1.8242245747996624, memory: 12954.58203125, disk: 24.274490356445313, network: 117.28749752044678 }
Network for node cr-bun: Network { available: 0.5, bandwidth: 0.5, latency: 573.4619761351496, packet_loss: 100.0 }


Node: cr-lsk, Coordinate: (515.9087667950914, 0.5779678274247689), Distance: 10.237349634383634
Node: cr-jhb, Coordinate: (2309906.7480582893, 1.528965623961135), Distance: 2309401.072871551
Node: cr-bun, Coordinate: (505.67527784880565, 0.5848768955821101), Distance: 0.28471018214365024
Node: cr-kla, Coordinate: (574.7206340818933, 0.3001667279122005), Distance: 69.04544701657994
Node: cr-dar, Coordinate: (505.67518706531337, 0.8703524271376406), Distance: 0.5701856992254402




      # JAEGER_DISABLED: "true"
      # SPAN_STORAGE_TYPE: "grpc-plugin"
      # GRPC_STORAGE_PLUGIN_BINARY: "/app/jaeger-mongodb"
      # GRPC_STORAGE_PLUGIN_CONFIGURATION_FILE: "/app/configs/config.yaml"





      ToDo - September 9 - 16

      * Only deploy the stack if considerable changes
            * Create a yaml file of ONLY services that need to be re-deployed
            - Keep track of service/node assignments and only deploy if there are changes *** placement > compare
      * Balance the node resource utilization
            * Helps to distribute the services
      * Placement | Service Tree, Node Tree, Resource Tree


      ***** Optimization Problem? What are the objectives? What are the constraints? 
      - In the a priori method, sufficient information must be provided before making any decision. This information can aggregate all the objectives into a single one by defining the single-objective function as a weighted sum of the normalized costs associated with each objective [35]. AKA - Weighted Sum Method / Scalarization method


    \begin{equation}\label{eq:all}
        \text{\textbf{\textit{Minimize}}} \sum_{i=1}^{k} \sum_{\substack{i=1 \\ j=1}}^m l_{ij}x_{ij} + \sum_{i=1}^{k} \sum_{j=1}^m y_{ij}S^{util}_{j}c_{i} + \sum_{i=1}^{k} \left( \sum_{j=1}^m y_{ij}\lambda_{i}S^{util}_{j} - \frac {(\frac{1}{k} \sum_{i=1}^{k} \sum_{j=1}^m y_{ij}S^{util}_{j})}{\alpha} \right)^2
    \end{equation}


```

### VM Setup

for pkg in docker.io docker-doc docker-compose docker-compose-v2 podman-docker containerd runc; do sudo apt-get remove $pkg; done


# Add Docker's official GPG key:
sudo apt-get update
sudo apt-get install ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc

# Add the repository to Apt sources:
echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu \
  $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

sudo apt-get update


sudo apt-get install docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin


sudo usermod -aG docker $USER


On the master, initialize the stack:

docker swarm init --advertise-addr 10.10.1.6

On the workers:

docker swarm join --token SWMTKN-1-1ncbid4ay0oajmjxrg047wd59vdvoc28kz91f4uizyn8x8r7rd-6i3z1p5m2fk4xk65cp88kllgo 10.10.1.6:2377


Label the nodes:
docker node update --label-add name=ocas01 mtbdoitzu4vr1sxdwgn26lgrx
docker node update --label-add name=ocas02 un0kgk87q71hpnuwn7l7dhol5
docker node update --label-add name=ocas03 sj2xbzfdjswmq3ldams5il9fm
docker node update --label-add name=ocas04 xufjp6855mey4wmnpnrfi5bol
docker node update --label-add name=ocas05 02inlnm7tzx3lnuaiz5o6atn6





*/5 * * * * /bin/bash /proj/rip-PG0/ocas/scripts/yonga/perf.sh 10.10.1.X >> /var/log/ocas-perf.log 2>&1 && sudo mv /users/mwotila/ocas/evaluation/network/metrics.txt /var/lib/node_exporter/yonga.prom


sudo mkdir /var/lib/node_exporter

mkdir -p /users/mwotila/ocas/evaluation/network

pip3 install tcp-latency
sudo apt install python3-pip iperf3 jq pkg-config libclang-dev libssl-dev

sudo ln -s .local/bin/tcp-latency /usr/local/bin/
sudo ln -s .local/bin/tcp-latency /usr/bin/

sudo touch /var/log/ocas-perf.log

sudo chown mwotila:cranecloud-PG0 /var/log/ocas-perf.log


sudo vim /etc/systemd/system/iperf3.service


[Unit]
Description=iperf3 server
After=syslog.target network.target auditd.service

[Service]
ExecStart=/usr/bin/iperf3 -s

[Install]
WantedBy=multi-user.target



sudo systemctl enable iperf3
sudo systemctl start iperf3

On all the workers

cd /proj/cranecloud-PG0/ocas/hotelReservation

docker compose build

docker tag hotel_reserv_profile_single_node:latest mwotila/hotel_reserv_profile_single_node:latest
docker tag hotel_reserv_geo_single_node:latest mwotila/hotel_reserv_geo_single_node:latest
docker tag hotel_reserv_search_single_node:latest mwotila/hotel_reserv_search_single_node:latest
docker tag hotel_reserv_rate_single_node:latest mwotila/hotel_reserv_rate_single_node:latest
docker tag hotel_reserv_rsv_single_node:latest mwotila/hotel_reserv_rsv_single_node:latest
docker tag hotel_reserv_frontend_single_node:latest mwotila/hotel_reserv_frontend_single_node:latest
docker tag hotel_reserv_recommend_single_node:latest mwotila/hotel_reserv_recommend_single_node:latest
docker tag hotel_reserv_user_single_node:latest mwotila/hotel_reserv_user_single_node:latest

docker rmi $(docker images -f "dangling=true" -q)

docker image rm hotel_reserv_recommend_single_node hotel_reserv_frontend_single_node hotel_reserv_profile_single_node hotel_reserv_rsv_single_node hotel_reserv_rate_single_node hotel_reserv_user_single_node hotel_reserv_geo_single_node

docker stack deploy -c monitor.yaml monitor


sudo mkdir /var/mongodb
sudo chown mwotila:cranecloud-PG0 /var/mongodb/


docker node update --label-add name=ocas01 mtbdoitzu4vr1sxdwgn26lgrx
docker node update --label-add name=ocas02 un0kgk87q71hpnuwn7l7dhol5
docker node update --label-add name=ocas03 sj2xbzfdjswmq3ldams5il9fm
docker node update --label-add name=ocas04 xufjp6855mey4wmnpnrfi5bol
docker node update --label-add name=ocas05 02inlnm7tzx3lnuaiz5o6atn6



Wrk2

sudo apt-get install luarocks
sudo luarocks install luasocket
lua -e "require('socket.core')"



Docker Stack 


Tcp-latency - https://pypi.org/project/tcp-latency/
sudo apt install python3-pip iperf3
pip install tcp-latency 


Scripts / Crontab


Rust - ocas

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh 

source $HOME/.cargo/env

docker rmi hotel_reserv_geo_single_node hotel_reserv_frontend_single_node hotel_reserv_rate_single_node  hotel_reserv_user_single_node hotel_reserv_rsv_single_node  hotel_reserv_recommend_single_node hotel_reserv_profile_single_node hotel_reserv_search_single_node
