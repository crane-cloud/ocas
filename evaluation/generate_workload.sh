#!/usr/bin/env bash
set -euo pipefail

##############################################
# 1. Read Protocol Name from Argument
##############################################

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <protocol>"
    echo "Example: $0 yonga"
    exit 1
fi

PROTOCOL="$1"

##############################################
# 2. Configurable Parameters
##############################################

URL="http://10.10.1.2:5000"
THREADS=8
CONNECTIONS=200
DURATION=45
WARMUP=10
# LOADS=(2000 4000 6000 8000 10000 12000 14000 16000 18000 20000)
#Low loads with a maximum of 500 RPS
LOADS=(10 50 100 150 200 250 300 350 400 450 500 1000 2000 4000 6000 8000 10000 12000 14000 16000 18000 20000)

##############################################
# 3. Output Directory
##############################################

OUTDIR="${PROTOCOL}_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUTDIR"

CSV="${OUTDIR}/${PROTOCOL}_results.csv"

echo "load_rps,req_per_sec,p50_ms,p90_ms,p99_ms,errors" > "$CSV"

##############################################
# 4. Helper: Normalize Latencies
##############################################

normalize() {
    sed 's/s$//g; s/ms$//g'
}

##############################################
# 5. Helper: Parse WRK Output
##############################################

parse_output() {
    local file="$1"

    local rps=$(grep "Requests/sec" "$file" | awk '{print $2}')

    # extract second column = latency value
    local p50=$(grep -E "50\.000%" "$file" | awk '{print $2}' | normalize)
    local p90=$(grep -E "90\.000%" "$file" | awk '{print $2}' | normalize)
    local p99=$(grep -E "99\.000%" "$file" | awk '{print $2}' | normalize)

    # extract error counts more accurately
    local errors=$(grep -E "Socket errors" "$file" | \
        awk -F':' '{print $2}' | tr -cd '0-9')

    echo "$rps,$p50,$p90,$p99,$errors"
}

##############################################
# 6. Run WRK2 Load Tests
##############################################

echo "Running WRK2 tests for protocol: $PROTOCOL"
for load in "${LOADS[@]}"; do
    echo "---------------------------------------------"
    echo "Protocol: $PROTOCOL | Load: $load RPS"
    echo "---------------------------------------------"

    outfile="$OUTDIR/${PROTOCOL}_${load}.txt"

    ##############################################
    # Warmup
    ##############################################
    echo "Warmup for ${WARMUP}s..."
    ./wrk-arm64 \
        -t${THREADS} \
        -c${CONNECTIONS} \
        -d${WARMUP}s \
        -R ${load} \
        "$URL" > /dev/null

    ##############################################
    # Main Execution
    ##############################################
    echo "Running main test..."
    ./wrk-arm64 \
        -t${THREADS} \
        -c${CONNECTIONS} \
        -d${DURATION}s \
        -R ${load} \
        --latency \
        "$URL" > "$outfile" 2>&1

    metrics=$(parse_output "$outfile")

    # Append to CSV
    echo "${load},${metrics}" >> "$CSV"

    echo "Saved: $outfile"
done

echo "==============================================="
echo "Protocol $PROTOCOL completed."
echo "Results stored in: $OUTDIR"
echo "CSV summary:        $CSV"
echo "==============================================="