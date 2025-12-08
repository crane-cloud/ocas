#!/bin/bash
tc qdisc add dev ens1f1np1 root handle 1: tbf rate 1gbit buffer 1600 limit 3000
tc qdisc add dev ens1f1np1 parent 1:1 handle 10: netem delay 129ms loss 1%