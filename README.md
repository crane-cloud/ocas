# ocas
Crane Cloud OCAS - Placement for low resource settings based on Docker Swarm


## Installation of the hotelreservation application
To install the hotelreservation application (the assumption is that you are working from the home directory):

```docker stack deploy --compose-file hotelreservation.yml hotelreservation```

Installation of supporting packages:

```sudo apt install luarocks libssl-dev zlib-dev```

```sudo luarocks install luasocket```

Installation of the benchmarking tool

```cd /tmp```

```git clone https://github.com/giltene/wrk2.git```

```cd wrk2```

```make```

```cp wrk ~/ocas/```

```cd /tmp```

```git clone https://github.com/delimitrou/DeathStarBench.git```

```cd ~/ocas/```

```cp -r /tmp/DeathStarBench/hotelReservation/wrk2/scripts .```

Run a sample benchmark

```./wrk -t 10 -c 100 -d 10 -s ./scripts/hotel-reservation/mixed-workload_type_1.lua http://cr-dar.cranecloud.africa:5000 -R 2000 -L```

