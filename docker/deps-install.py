#!/usr/bin/env /usr/bin/python3.8

import argparse
import subprocess
import sys
import os

def run(*args):
    subprocess.run(args, check=True, shell=True)

def apt_install(name):
    run('apt-get --assume-yes install "{}"'.format(name))

def wget(name):
    run('wget "{}"'.format(name))

def tar_extract(name, z=False):
    if z:
        run('tar -xzf "{}"'.format(name))
    else:
        run('tar -xf "{}"'.format(name))

def link(name, target, symbolic=True):
    if symbolic:
        run('ln -s "{}" "{}"'.format(target, name))
    else:
        run('ln "{}" "{}"'.format(target, name))

def move(src, dest):
    run('mv "{}" "{}"'.format(src, dest))

print('Installing wget utility')
apt_install('wget')

print('Installing curl utility')
apt_install('curl')

print('Installing make utility')
apt_install('make')


print('Installing gcc')
apt_install('gcc')

print('Installing g++')
apt_install('g++')

print('Installing clang')
apt_install('clang')


# Install Java distributions.
def install_java():
    print('Downloading Java binary archive')
    wget('https://download.java.net/java/GA/jdk13.0.1/cec27d702aa74d5a8630c65ae61e4305/9/GPL/' +
        'openjdk-13.0.1_linux-x64_bin.tar.gz')
    tar_extract('openjdk-13.0.1_linux-x64_bin.tar.gz', z=True)
    os.remove('openjdk-13.0.1_linux-x64_bin.tar.gz')

    os.chdir('jdk-13.0.1')

    print('Installing Java binaries')
    link('/usr/bin/java', './bin/java')
    link('/usr/bin/javac', './bin/javac')
    link('/usr/bin/jar', './bin/jar')

    os.chdir('..')

os.mkdir('java')
os.chdir('java')
install_java()
os.chdir('..')


# Install rust distributions.
def install_rust(*versions):
    print('Installing rustup')
    run("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh")

    for v in versions:
        print('Installing rust toolchain version {}'.format(v))
        run('rustup toolchain install {}'.format(v))

install_rust('1.38', '1.39', '1.40')

print('Congratulations.')
