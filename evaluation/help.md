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
