# ocas
Crane Cloud OCAS - Placement for low resource settings based on Docker Swarm


## Installation of the hotelreservation application
To install the hotelreservation application (the assumption is that you are working from the home directory):

```docker stack deploy --compose-file hotelreservation.yml hotelreservation```

You should check the status of the services and ensure that replication is correctly being done and the numbers running as expected:

```docker stack ls```

```docker service ls```

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






Other tests

```curl 'http://cr-dar.cranecloud.africa:5000/reservation?inDate=2015-04-19&outDate=2015-04-24&lat=nil&lon=nil&hotelId=9&customerName=Cornell_1&username=Cornell_1&password=1111111111&number=1'```


```user@client0:~$ curl 'http://cr-dar.cranecloud.africa:5000/user?username=Cornell_1&password=1111111111'```


To build the jaeger-mongodb library compatible with Alpine Linux - used by Jaeger

```GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build ./cmd/jaeger-mongodb```