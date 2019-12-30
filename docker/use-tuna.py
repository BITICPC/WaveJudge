#!/usr/bin/env python3

import argparse
import sys
import subprocess

def get_args():
    parser = argparse.ArgumentParser(
        description='This script replaces package repository to TUNA by option.')
    parser.add_argument('--tuna', default='yes', required=False,
        help='Should change package repository to TUNA.')
    return parser.parse_args()

args = get_args()
if args.tuna != 'yes':
    sys.exit(0)

# Install apt-transport-https to fetch packages from TUNA source. This package should be installed
# from the official debian source.
proc = subprocess.run('apt-get --assume-yes update', shell=True, check=True)
proc = subprocess.run('apt-get --assume-yes install apt-transport-https', shell=True, check=True)

tuna_sources = [
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-updates main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-backports main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian-security stretch/updates main contrib non-free'
]

with open('/etc/apt/sources.list', mode='w') as fp:
    for ln in tuna_sources:
        print(ln, file=fp)
