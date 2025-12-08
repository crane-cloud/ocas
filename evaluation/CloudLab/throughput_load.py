#!/usr/bin/env python3
"""
throughput_load.py

Traverse subdirectories (each subdir = a protocol), find *_results.csv files,
read load_rps and req_per_sec, aggregate across runs (group by load_rps),
compute mean and std, and plot Load vs Throughput with error bars.

Usage:
    python3 throughput_load.py --root . --out throughput_by_protocol.pdf --show

Dependencies:
    pip install pandas matplotlib numpy
"""

from pathlib import Path
import argparse
import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.ticker import FuncFormatter


# -------------------------------------------------------------------
# Tick formatter: convert 1000 → 1K, 23000 → 23K
# -------------------------------------------------------------------
def format_k_ticks(x, pos):
    try:
        if x >= 1_000_000:
            return f"{int(x/1_000_000)}M"
        if x >= 1_000:
            return f"{int(x/1000)}K"
        return f"{int(x)}"
    except Exception:
        return str(x)


# -------------------------------------------------------------------
# Utility: find all *_results.csv files grouped by protocol directory
# -------------------------------------------------------------------
def find_result_files(root: Path):
    protocols = {}
    for child in sorted(root.iterdir()):
        if not child.is_dir():
            continue

        proto = child.name
        csvs = list(child.rglob("*_results.csv"))

        # fallback names like "yonga_results.csv"
        if not csvs:
            csvs = list(child.rglob("*results.csv"))

        if csvs:
            protocols[proto] = csvs

    return protocols


# -------------------------------------------------------------------
# Read and normalize CSV files
# -------------------------------------------------------------------
def read_results(csv_paths):
    dfs = []
    for p in csv_paths:
        try:
            df = pd.read_csv(p, dtype=str, keep_default_na=False)
        except Exception as e:
            print(f"Warning: could not read {p}: {e}")
            continue

        df.columns = [c.strip() for c in df.columns]

        # Normalize expected column names
        if "load_rps" not in df.columns and "load" in df.columns:
            df = df.rename(columns={"load": "load_rps"})

        if "req_per_sec" not in df.columns:
            for alt in ["reqs_per_sec", "requests_per_sec", "requests/sec", "Requests/sec"]:
                if alt in df.columns:
                    df = df.rename(columns={alt: "req_per_sec"})
                    break

        if "load_rps" not in df.columns or "req_per_sec" not in df.columns:
            print(f"Skipping {p} (missing load_rps or req_per_sec)")
            continue

        # Clean numerics
        df["req_per_sec"] = (
            df["req_per_sec"]
            .astype(str)
            .str.replace(",", "")
            .str.strip()
        )
        df["req_per_sec"] = pd.to_numeric(df["req_per_sec"], errors="coerce")

        df["load_rps"] = (
            df["load_rps"]
            .astype(str)
            .str.replace(",", "")
            .str.strip()
        )
        df["load_rps"] = pd.to_numeric(df["load_rps"], errors="coerce")

        df = df.dropna(subset=["load_rps", "req_per_sec"])

        dfs.append(df[["load_rps", "req_per_sec"]])

    if not dfs:
        return pd.DataFrame(columns=["load_rps", "req_per_sec"])

    return pd.concat(dfs, ignore_index=True)


# -------------------------------------------------------------------
# Aggregate values per load level
# -------------------------------------------------------------------
def aggregate_for_protocol(df):
    if df.empty:
        return pd.DataFrame(columns=["load_rps", "mean", "std", "count", "values"])

    grouped = (
        df.groupby("load_rps")["req_per_sec"]
        .agg(list)
        .reset_index()
        .rename(columns={"req_per_sec": "values"})
    )

    grouped["mean"] = grouped["values"].apply(lambda l: float(np.nanmean(l)))
    grouped["std"] = grouped["values"].apply(
        lambda l: float(np.nanstd(l, ddof=1)) if len(l) > 1 else 0.0
    )
    grouped["count"] = grouped["values"].apply(len)

    return grouped.sort_values("load_rps")[
        ["load_rps", "mean", "std", "count", "values"]
    ]


# -------------------------------------------------------------------
# Plotting
# -------------------------------------------------------------------
def plot_protocols(aggregates, out_path=None, show=False):

    # Safe fonts
    plt.rcParams.update({
        "font.family": "sans-serif",
        "font.sans-serif": "DejaVu Sans",
        "font.size": 8,
        "legend.handlelength": 1,
        "axes.titlepad": 5,
    })

    # Protocol → color mapping
    protocol_colors = {
        "YONGA": "#999999",
        "SPREAD": "#e41a1c",
        "RANDOM": "#984ea3",
        "BINPACK": "#a65628",
        "GREEDY-BFD": "#377eb8",
    }

    # Protocol → marker mapping
    protocol_markers = {
        "YONGA": "d",
        "SPREAD": "+",
        "RANDOM": "o",
        "BINPACK": "*",
        "GREEDY-BFD": "s",
    }

    fallback_color = "#377eb8"
    fallback_marker = "o"

    plt.figure(figsize=(8, 4))
    ax = plt.gca()

    # APPLY "K" FORMATTER HERE
    ax.xaxis.set_major_formatter(FuncFormatter(format_k_ticks))
    ax.yaxis.set_major_formatter(FuncFormatter(format_k_ticks))

    for proto, agg in aggregates.items():

        if agg.empty:
            continue

        proto_upper = proto.upper()

        color = protocol_colors.get(proto_upper, fallback_color)
        marker = protocol_markers.get(proto_upper, fallback_marker)

        x = agg["load_rps"].values
        y = agg["mean"].values
        yerr = agg["std"].values

        # Line + marker
        ax.plot(
            x, y,
            label=proto_upper,
            color=color,
            marker=marker,
            markersize=3,
            linewidth=1,
        )

        # Error bars
        ax.errorbar(
            x, y,
            yerr=yerr,
            fmt="none",
            ecolor="black",
            elinewidth=1,
            capsize=3,
        )

    ax.set_xlabel("Load (Requests/Second)", fontsize=10)
    ax.set_ylabel("Throughput (Requests/Second)", fontsize=10)
    ax.grid(True, linestyle="--", linewidth=0.5, alpha=0.2)
    ax.legend(loc="upper left", frameon=False, fontsize=8)
    ax.set_xlim(left=0)

    plt.tight_layout()

    if out_path:
        plt.savefig(out_path, dpi=300)
        print(f"Saved: {out_path}")

    if show:
        plt.show()

    plt.close()


# -------------------------------------------------------------------
# Main
# -------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser(description="Plot load vs throughput per protocol")
    parser.add_argument("--root", "-r", type=str, default=".", help="Root directory containing protocol folders")
    parser.add_argument("--out", "-o", type=str, default="throughput_by_protocol.pdf", help="Output PDF filename")
    parser.add_argument("--show", action="store_true", help="Show the plot interactively")
    args = parser.parse_args()

    root = Path(args.root).expanduser().resolve()

    protocols = find_result_files(root)
    if not protocols:
        print(f"No *_results.csv files found under {root}")
        return

    aggregates = {}

    for proto, csvs in protocols.items():
        print(f"Protocol '{proto}': found {len(csvs)} files")

        df = read_results(csvs)
        agg = aggregate_for_protocol(df)

        print(f" → Aggregated loads: {list(agg['load_rps'].astype(int).values)} ({len(agg)} rows)")
        aggregates[proto] = agg

    plot_protocols(aggregates, out_path=args.out, show=args.show)


if __name__ == "__main__":
    main()