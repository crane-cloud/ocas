#!/bin/bash

LOGFILE="ocas_wrk_results.log"
echo "Starting test run at $(date)" > "$LOGFILE"

for i in $(seq 1 6); do
  echo "Iteration #$i: Running 'ocas' command" | tee -a "$LOGFILE"
  ./target/debug/ocas -m ../docker-compose.yaml -p spread -c ../evaluation/config.yaml -u http://127.0.0.1:32000 -s hotelreservation | tee -a "$LOGFILE"

  echo "Waiting for 1 minute before running 'wrk' tests..." | tee -a "$LOGFILE"
  sleep 60

  echo "Running 5 'wrk' tests" | tee -a "$LOGFILE"
  for j in $(seq 1 5); do
    echo "  Test #$j" | tee -a "$LOGFILE"
    ../wrk -t 1 -c 10 -d 30 -s ../scripts/hotel-reservation/mixed-workload_type_1.lua http://cr-lsk.cranecloud.africa:5000 -R 1000000 -L | grep 'Requests/sec' | tee -a "$LOGFILE"
  done

  echo "All tests completed for iteration #$i!" | tee -a "$LOGFILE"
done

echo "Test run completed at $(date)" | tee -a "$LOGFILE"
