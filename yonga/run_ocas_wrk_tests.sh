#!/bin/bash

for i in {1..13}
do
  echo "Iteration #$i: Running 'ocas' command"
  ./target/debug/ocas -m ../docker-compose.yaml -p binpack -c ../evaluation/config.yaml -u http://127.0.0.1:32000 -s hotelreservation
  
  echo "Waiting for 1 minute before running 'wrk' tests..."
  sleep 60
  
  echo "Running 5 'wrk' tests"
  for j in {1..5}
  do
    echo "  Test #$j"
    ../wrk -t 1 -c 10 -d 30 -s ../scripts/hotel-reservation/mixed-workload_type_1.lua http://cr-lsk.cranecloud.africa:5000 -R 1000000 -L | grep 'Requests/sec'
  done
done

echo "All tests completed!"
