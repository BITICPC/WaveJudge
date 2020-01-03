#!/usr/bin/env /usr/bin/python3.8

import subprocess

def run(args):
    subprocess.run(args, shell=True, check=True)

def apt_install(name):
    run('apt-get --assume-yes install "{}"'.format(name))

def apt_update():
    run('apt-get --assume-yes update')

apt_update()
apt_install('apt-transport-https')

tuna = [
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-updates main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian/ stretch-backports main contrib non-free',
    'deb https://mirrors.tuna.tsinghua.edu.cn/debian-security stretch/updates main contrib non-free'
]

with open('/etc/apt/sources.list', 'w') as fp:
    for line in tuna:
        print(line, file=fp)
