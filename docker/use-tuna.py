#!/usr/bin/env python3

import sys
import subprocess

# Install apt-transport-https to fetch packages from TUNA source. This package should be installed
# from the official debian source.
proc = subprocess.run(['apt', 'update'], check=True)
proc = subprocess.run(['apt', 'install', 'apt-transport-https'], check=True)

tuna_sources = [
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-updates main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-backports main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian-security stretch/updates main contrib non-free'
]

with open('/etc/apt/sources.list', mode='w') as fp:
    for ln in tuna_sources:
        print(ln, file=fp)
