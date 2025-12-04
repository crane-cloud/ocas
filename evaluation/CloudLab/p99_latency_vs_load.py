#!/usr/bin/env python3
"""
p99_latency_vs_load.py

Traverse protocol folders, read *_results.csv files,
extract (load_rps, p99_ms), compute mean/std for each load,
and plot p99 latency vs load using log-scale on Y-axis.

Usage:
    python3 p99_latency_vs_load.py --root . --out p99_latency_logscale.pdf --show
"""

from pathlib import Path
import argparse
import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.ticker import FuncFormatter   # <-- Added

# ------------------------------------------
# Tick formatter for X-axis (K, M, no decimals)
# ------------------------------------------
def format_k_ticks(x, pos):
    if x >= 1_000_000:
        return f"{int(x/1_000_000)}M"
    elif x >= 1_000:
        return f"{int(x/1_000)}K"
    else:
        return f"{int(x)}"

# ------------------------------------------
# Matplotlib style
# ------------------------------------------
plt.rcParams.update({
    'font.family': 'sans-serif',
    'font.sans-serif': ['DejaVu Sans'],
    'font.size': 8,
    'legend.handlelength': 1,
    'axes.titlepad': 5
})

protocol_colors = {
    'YONGA': '#999999',
    'SPREAD': '#e41a1c',
    'RANDOM': '#984ea3',
    'BINPACK': '#a65628',
    'GREEDY-BFD': '#377eb8'
}

protocol_markers = {
    "YONGA": 'd',
    "SPREAD": '+',
    "RANDOM": 'o',
    "BINPACK": '*',
    "GREEDY-BFD": 's'
}

# ------------------------------------------
# Helpers
# ------------------------------------------

def find_result_files(root: Path):
    protocols = {}
    for child in sorted(root.iterdir()):
        if child.is_dir():
            csvs = list(child.rglob('*_results.csv'))
            if csvs:
                protocols[child.name.upper()] = csvs
    return protocols


def parse_latency(x):
    """Convert latency values to milliseconds."""
    x = str(x).strip()
    if x.endswith('m'):
        return float(x.replace('m', ''))
    else:
        try:
            return float(x) * 1000.0
        except:
            return np.nan


def read_results(csv_paths):
    dfs = []
    for p in csv_paths:
        try:
            df = pd.read_csv(p, dtype=str, keep_default_na=False)
        except Exception as e:
            print(f"Warning: cannot read {p}: {e}")
            continue

        df.columns = [c.strip() for c in df.columns]

        if 'load_rps' not in df.columns and 'load' in df.columns:
            df = df.rename(columns={'load': 'load_rps'})

        if 'load_rps' not in df.columns or 'p99_ms' not in df.columns:
            print(f"Skipping {p}, missing required columns.")
            continue

        df['load_rps'] = pd.to_numeric(
            df['load_rps'].str.replace(',', ''), errors='coerce'
        )
        df['p99_ms'] = df['p99_ms'].apply(parse_latency)

        df = df.dropna(subset=['load_rps', 'p99_ms'])
        dfs.append(df[['load_rps', 'p99_ms']])

    if not dfs:
        return pd.DataFrame(columns=['load_rps', 'p99_ms'])

    return pd.concat(dfs, ignore_index=True)


def aggregate(df):
    """Compute mean + std of p99 per load."""
    if df.empty:
        return pd.DataFrame(columns=['load_rps', 'mean', 'std', 'count'])

    grp = df.groupby('load_rps')['p99_ms'].agg(list).reset_index()
    grp['mean'] = grp['p99_ms'].apply(lambda x: np.mean(x))
    grp['std'] = grp['p99_ms'].apply(lambda x: np.std(x, ddof=1) if len(x) > 1 else 0)
    grp['count'] = grp['p99_ms'].apply(len)

    return grp.sort_values('load_rps')

# ------------------------------------------
# Plotting
# ------------------------------------------
def plot_p99(aggregates, out_path=None, show=False):
    plt.figure(figsize=(8, 4))
    ax = plt.gca()

    for proto, df in aggregates.items():
        if df.empty:
            continue

        x = df['load_rps'].values
        y = df['mean'].values
        yerr = df['std'].values

        ax.errorbar(
            x,
            y,
            yerr=yerr,
            color=protocol_colors.get(proto, '#333333'),
            marker=protocol_markers.get(proto, 'o'),
            linestyle='-',
            linewidth=1,
            markersize=2,
            capsize=3,
            ecolor="black",
            elinewidth=0.9,
            label=proto,
        )

    # Axis labels and scales
    ax.set_xlabel("Load (Requests per Second)")
    ax.set_ylabel("p99 Latency (ms) [log scale]")
    ax.set_yscale("log")

    # ----- APPLY X-axis K/M formatting -----
    ax.xaxis.set_major_formatter(FuncFormatter(format_k_ticks))

    ax.grid(True, linestyle='--', linewidth=0.5, alpha=0.3)
    ax.legend(frameon=False, fontsize=7)

    plt.tight_layout()

    if out_path:
        plt.savefig(out_path, dpi=200)
        print(f"Saved: {out_path}")

    if show:
        plt.show()

    plt.close()

# ------------------------------------------
# Main
# ------------------------------------------
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", "-r", default=".", help="Root directory containing protocol folders")
    parser.add_argument("--out", "-o", default="p99_latency_logscale.pdf", help="Output plot PDF")
    parser.add_argument("--show", action="store_true")
    args = parser.parse_args()

    root = Path(args.root).resolve()

    protocols = find_result_files(root)
    if not protocols:
        print("No protocol folders found.")
        return

    aggregates = {}
    for proto, csvs in protocols.items():
        df = read_results(csvs)
        agg = aggregate(df)
        aggregates[proto] = agg
        print(f"{proto}: {len(csvs)} runs, {len(agg)} load points")

    plot_p99(aggregates, args.out, args.show)


if __name__ == "__main__":
    main()