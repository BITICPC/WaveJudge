#!/usr/bin/env python3

import argparse
import subprocess
import sys
import os

def get_args():
    parser = argparse.ArgumentParser(description='Install all required dependencies of WaveJudge')
    parser.add_argument('--tuna', nargs='?', default='yes',
        help='Use TUNA repository instead of the official package repository.')
    return parser.parse_args()

def run(*args):
    subprocess.run(args, check=True, shell=True)

def apt_install(name):
    run('apt install "{}"'.format(name))

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

args = get_args()
if args.tuna == 'yes':
    print('Use TUNA package repository')
    run('./scripts/use-tuna.py')
run('apt update')


print('Installing wget utility')
apt_install('wget')

print('Installing curl utility')
apt_install('curl')

print('Installing make utility')
apt_install('make')


print('Installing gcc-8')
apt_install('gcc-8')
link('/usr/bin/gcc', '/usr/bin/gcc-8')

print('Installing g++-8')
apt_install('g++-8')
link('/usr/bin/g++', '/usr/bin/g++-8')

print('Installing clang-8')
apt_install('clang-8')
link('/usr/bin/clang', '/usr/bin/clang-8')
link('/usr/bin/clang++', '/usr/bin/clang++-8')

# Install python distributions.s
def install_python(version, build_jobs=4):
    print('Downloading python {} source code'.format(version))
    wget('https://www.python.org/ftp/python/{}/Python-{}.tar.xz'.format(version, version))
    tar_extract('Python-{}.tar.xz'.format(version))
    os.remove('Python-{}.tar.xz'.format(version))
    move('Python-{}'.format(version), version)

    os.chdir(version)

    print('Building python {} from source'.format(version))
    run('./configure')
    run('make -j{}'.format(build_jobs))

    simp_version = '.'.join(version.split('.')[:2])
    print('Installing python {} to python{}'.format(version, simp_version))
    link('/usr/bin/python{}', './python'.format(simp_version))

    os.chdir('..')

os.mkdir('python')
os.chdir('python')
install_python('3.6.10')
install_python('3.7.6')
install_python('3.8.1')
os.chdir('..')

# Install Java distributions.
def install_java():
    print(f'Downloading Java binary archive')
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
