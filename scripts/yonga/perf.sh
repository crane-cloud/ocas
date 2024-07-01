#!/bin/bash

# Define the JSON file path
json_file="/home/ubuntu/ocas/evaluation/network/metrics.txt"
temp_file="/home/ubuntu/ocas/evaluation/network/metrics_temp.txt"

# Define a list of replicas
replicas=("129.232.230.130" "196.32.212.213" "102.134.147.244" "196.32.215.213" "196.43.171.248")

# Check if an IP address argument is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <ip_address>"
    exit 1
else
    # Check if the provided IP address is valid
    ip=$1
    if [[ ! $ip =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "Invalid IP address: $ip"
        exit 1
    fi

    # Check if the provided IP address is in the list of replicas
    if [[ ! " ${replicas[@]} " =~ " $ip " ]]; then
        echo "IP address $ip is not in the list of replicas"
        exit 1
    fi
fi

measure_bandwidth_pl_latency() {
    local idx=$1
    local ip=$2

    id=$((idx+1))

    latency_output=$(tcp-latency -p 9100 $ip)
    #latency_output=$(/home/ubuntu/.pyenv/shims/tcp-latency -p 9100 $ip)
    last_line_log=$(echo "$latency_output" | tail -n 1)

    if [[ "$last_line_log" == *"All 5 transmissions failed"* ]]; then
        echo "Failed to measure latency to $ip"
        avg_latency=1000
        availability=0
    else
        echo "Successfully measured latency to $ip"
        avg_latency=$(echo "$latency_output" | awk '/rtt min\/avg\/max/ {split($0, a, "="); split(a[2], b, "/"); value=b[2]; unit=b[3]; if (unit == "s") value *= 1000; else if (unit == "ns") value /= 1000000; print value}')

        availability=1
    fi

    iperf3_output=$(iperf3 -c $ip -u -b 10M -t 5 --json)

    average_bandwidth_bps=$(echo $iperf3_output | jq -r '.end.sum.bits_per_second')
    packet_loss=$(echo $iperf3_output | jq -r '.end.sum.lost_percent')
    average_bandwidth_mbps=$(echo "scale=2; $average_bandwidth_bps / 1000000" | bc)

    if [ "$average_bandwidth_mbps" == "0" ] && [ "$packet_loss" == "null" ]; then
        average_bandwidth_mbps=0.00001
        packet_loss=100
    fi

    echo "Bandwidth to $ip: $average_bandwidth_mbps Mbps"
    echo "Packet loss to $ip: $packet_loss %"
    echo "Latency to $ip: $avg_latency ms"
    echo "Availability: $availability"

    id_string="$id"
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Write to the temporary JSON file
    if [ -f "$json_file" ]; then
        cp "$json_file" "$temp_file"
    else
        echo "" > "$temp_file"
    fi

    # Append bandwidth, latency, packet loss and availability measurements to the temporary text file
    echo "bandwidth{ip=\"$ip\",timestamp=\"$timestamp\",metric=\"bandwidth\"} $average_bandwidth_mbps" >> "$temp_file"
    echo "packet_loss{ip=\"$ip\",timestamp=\"$timestamp\",metric=\"packet_loss\"} $packet_loss" >> "$temp_file"
    echo "latency{ip=\"$ip\",timestamp=\"$timestamp\",metric=\"latency\"} $avg_latency" >> "$temp_file"
    echo "availability{ip=\"$ip\",timestamp=\"$timestamp\",metric=\"availability\"} $availability" >> "$temp_file"

    # Clear the original text file
    echo "" > "$json_file"

    # Move the temporary text file to the original text file
    mv "$temp_file" "$json_file"
}

for i in "${!replicas[@]}"; do
    replica="${replicas[$i]}"
    if [ "$replica" == "$ip" ]; then
        echo "Skipping test to the local server ($ip)."
    else
        echo "Measuring metrics for replica at index $i: $replica..."
        measure_bandwidth_pl_latency "$i" "$replica"
        echo ""
    fi
done