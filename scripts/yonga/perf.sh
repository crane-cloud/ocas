#!/bin/bash

# Define the JSON file path
json_file="/home/ubuntu/ocas/evaluation/profiles/metrics.json"
temp_file="/home/ubuntu/ocas/evaluation/profiles/metrics_temp.json"

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
    last_line_log=$(echo "$latency_output" | tail -n 1)

    if [[ "$last_line_log" == *"All 5 transmissions failed"* ]]; then
        echo "Failed to measure latency to $ip"
        avg_latency=1000
        cpu_percentage=100
        memory_utilization=100
        disk_utilization=100
        availability=0
    else
        echo "Successfully measured latency to $ip"
        avg_latency=$(echo "$latency_output" | awk '/rtt min\/avg\/max/ {split($0, a, "="); split(a[2], b, "/"); value=b[2]; unit=b[3]; if (unit == "s") value *= 1000; else if (unit == "ns") value /= 1000000; print value}')

        node_exporter_metrics=$(curl -s http://$ip:9100/metrics)

        cpu_utilization=$(echo "$node_exporter_metrics" | grep '^node_cpu_seconds_total' | grep -v 'mode="idle"' | grep -v 'mode="iowait"' | grep -v 'mode="irq"' | grep -v 'mode="softirq"' | grep -v 'mode="steal"' | awk '{sum += $2} END {print sum}')
        total_time=$(grep '^node_cpu_seconds_total' <<< "$node_exporter_metrics" | grep 'mode="idle"' | awk '{sum += $2} END {print sum}')
        total_time=$(printf "%.0f" $total_time)

        if [ "$total_time" -ne 0 ]; then
            cpu_percentage=$(awk -v total_time="$total_time" -v cpu_utilization="$cpu_utilization" 'BEGIN {print (cpu_utilization / total_time) * 100}')
        else
            cpu_percentage=0
        fi

        mem_available=$(echo "$node_exporter_metrics" | grep '^node_memory_MemAvailable_bytes' | awk '{print $2}')
        mem_total=$(echo "$node_exporter_metrics" | grep '^node_memory_MemTotal_bytes' | awk '{print $2}')
        mem_total=$(printf "%.0f" $mem_total)

        if [ "$mem_total" -ne 0 ]; then
            memory_utilization=$(awk -v mem_available="$mem_available" -v mem_total="$mem_total" 'BEGIN {print ((mem_total - mem_available) / mem_total) * 100}')
        else
            memory_utilization=0
        fi

        filesystem_size=$(echo "$node_exporter_metrics" | grep '^node_filesystem_size_bytes' | grep 'mountpoint="/"' | awk '{print $2}')
        filesystem_avail=$(echo "$node_exporter_metrics" | grep '^node_filesystem_avail_bytes' | grep 'mountpoint="/"' | awk '{print $2}')
        filesystem_size=$(printf "%.0f" $filesystem_size)

        if [ "$filesystem_size" -ne 0 ]; then
            disk_utilization=$(awk -v filesystem_avail="$filesystem_avail" -v filesystem_size="$filesystem_size" 'BEGIN {print ((filesystem_size - filesystem_avail) / filesystem_size) * 100}')
        else
            disk_utilization=0
        fi

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
    echo "CPU utilization: $cpu_percentage %"
    echo "Memory utilization: $memory_utilization%"
    echo "Disk utilization: $disk_utilization %"
    echo "Availability: $availability"

    w_r=0.2
    w_n=0.7
    w_a=0.1
    w_cpu=0.5
    w_mem=0.4
    w_disk=0.1
    w_bw=0.3
    w_pl=0.1
    w_lat=0.6
    w_avail=1
    min_lat=20
    max_bw=10
    min_pl=0.00001

    profile=$(echo "scale=2; $w_r * ($w_cpu * (100 - $cpu_percentage) + $w_mem * (100 - $memory_utilization) + $w_disk * (100 - $disk_utilization)) \
    + $w_n * ($w_bw * ($average_bandwidth_mbps / $max_bw) + $w_pl * (100 - $packet_loss) + $w_lat * ($min_lat / $avg_latency)) \
        + $w_a * ($w_a * $availability)" | bc)

    echo "Profile for $ip: $profile"
    rounded_profile=$(printf "%.0f" "$profile")
    id_string="$id"
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Write to the temporary JSON file
    if [ -f "$json_file" ]; then
        cp "$json_file" "$temp_file"
    else
        echo "{}" > "$temp_file"
    fi

    # Append bandwidth and latency measurements to the temporary JSON file
    jq --arg time "$timestamp" --arg bandwidth "$average_bandwidth_mbps" --arg loss "$packet_loss" --arg latency "$avg_latency" \
        --arg cpu "$cpu_percentage" --arg memory "$memory_utilization" --arg disk "$disk_utilization" --arg availability "$availability" \
        --argjson profile "$rounded_profile" --arg id "$id_string" \
        '.[$id][$time] += {profile: $profile}' "$temp_file" > temp.json && mv temp.json "$temp_file"

    # Move the temporary JSON file to the original JSON file
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