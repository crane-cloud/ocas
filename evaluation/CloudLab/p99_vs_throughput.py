#!/usr/bin/env python3
"""
p99_vs_throughput.py

Traverse protocol directories, read *_results.csv files,
extract p99 latency and throughput (req_per_sec),
aggregate across runs, and plot:

    Throughput (RPS) vs p99 Latency (ms)

Usage:
    python3 p99_vs_throughput.py --root . --out p99_vs_throughput.pdf
"""

from pathlib import Path
import argparse
import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
import re

# ------------------------------------------------------------------
# Matplotlib and colors configuration
# ------------------------------------------------------------------
plt.rcParams.update({
    'font.family': 'sans-serif',
    'font.sans-serif': ['DejaVu Sans'],   # avoid Arial
    'font.size': 8,
    'legend.handlelength': 1.0,
    'axes.titlepad': 5,
})

protocol_colors = {
    'yonga':   '#999999',   # gray
    'spread':  '#e41a1c',   # red
    'random':  '#984ea3',   # purple
    'binpack': '#a65628',   # brown
}

protocol_markers = {
    'yonga': 'd',
    'spread': '+',
    'random': 'o',
    'binpack': '*',
}


# ------------------------------------------------------------------
# Utility: parse p99 values like "1.29m", "2.84", "540.67m", "1.18"
# ------------------------------------------------------------------
def parse_latency(value):
    """
    Convert latency text to milliseconds.
    - '1.29m'     → 1.29 ms
    - '540.67m'   → 540.67 ms
    - no suffix   → seconds → convert to ms
    """
    if value is None or value == "":
        return np.nan

    s = str(value).strip().lower()

    # pattern: number + optional unit
    m = re.match(r"([0-9]*\.?[0-9]+)([a-z]*)", s)
    if not m:
        return np.nan

    num = float(m.group(1))
    unit = m.group(2)

    if unit == 'm':  # milliseconds
        return num
    if unit in ('ms', ''):
        # If no unit → interpret as seconds
        if unit == '':
            return num * 1000.0
        return num
    if unit == 's':  # seconds
        return num * 1000.0

    return np.nan


# ------------------------------------------------------------------
# Find *_results.csv per protocol
# ------------------------------------------------------------------
def find_result_files(root: Path):
    protocols = {}
    for child in sorted(root.iterdir()):
        if not child.is_dir():
            continue
        proto = child.name.lower()
        csvs = list(child.rglob('*_results.csv'))
        if csvs:
            protocols[proto] = csvs
    return protocols


# ------------------------------------------------------------------
# Read CSVs and collect throughput + p99 latency
# ------------------------------------------------------------------
def read_results(csv_paths):
    dfs = []
    for p in csv_paths:
        try:
            df = pd.read_csv(p, dtype=str, keep_default_na=False)
        except:
            continue

        df.columns = [c.strip() for c in df.columns]

        # Required columns
        if not all(col in df.columns for col in ['req_per_sec','p99_ms']):
            continue

        # Clean throughput
        df['req_per_sec'] = df['req_per_sec'].str.replace(',', '').astype(float)

        # Parse p99 latency
        df['p99_ms'] = df['p99_ms'].apply(parse_latency)

        # Keep clean rows
        df = df.dropna(subset=['req_per_sec', 'p99_ms'])

        dfs.append(df[['req_per_sec', 'p99_ms']])

    if not dfs:
        return pd.DataFrame(columns=['req_per_sec','p99_ms'])

    return pd.concat(dfs, ignore_index=True)


# ------------------------------------------------------------------
# Aggregate: mean/std per throughput point
# ------------------------------------------------------------------
def aggregate(df):
    if df.empty:
        return pd.DataFrame(columns=['req_per_sec','mean','std'])

    gb = df.groupby('req_per_sec')['p99_ms'].agg(list).reset_index()
    gb['mean'] = gb['p99_ms'].apply(np.mean)
    gb['std']  = gb['p99_ms'].apply(lambda x: np.std(x, ddof=1) if len(x)>1 else 0.0)

    return gb.sort_values('req_per_sec')


# ------------------------------------------------------------------
# Plot p99 latency vs throughput
# ------------------------------------------------------------------
def plot_p99(aggregates, out_path):

    plt.figure(figsize=(8, 4))
    ax = plt.gca()

    for proto, agg in aggregates.items():
        if agg.empty:
            continue

        color = protocol_colors.get(proto, '#333333')
        marker = protocol_markers.get(proto, 'o')

        ax.errorbar(
            agg['req_per_sec'],
            agg['mean'],
            yerr=agg['std'],
            marker=marker,
            markersize=4,
            color=color,
            linestyle='-',
            linewidth=1.4,
            capsize=3,
            label=proto.upper()
        )

    ax.set_xlabel("Throughput (Requests/sec)")
    ax.set_ylabel("p99 Latency (ms)")
    ax.grid(True, linestyle="--", linewidth=0.5, alpha=0.3)
    ax.legend(frameon=False)
    plt.tight_layout()

    plt.savefig(out_path, dpi=250)
    print(f"Saved → {out_path}")
    plt.close()


# ------------------------------------------------------------------
# Main
# ------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", "-r", type=str, default=".", help="Root folder containing protocol subdirs")
    parser.add_argument("--out", "-o", type=str, default="p99_vs_throughput.pdf")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    protocols = find_result_files(root)
    if not protocols:
        print("No *_results.csv found.")
        return

    aggregates = {}

    for proto, csvs in protocols.items():
        print(f"[{proto}] reading {len(csvs)} CSVs…")
        df = read_results(csvs)
        agg = aggregate(df)
        aggregates[proto] = agg
        print(f"  → {len(agg)} points aggregated")

    plot_p99(aggregates, args.out)


if __name__ == "__main__":
    main()