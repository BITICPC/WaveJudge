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

def run(*args, check=True, shell=True):
    proc = subprocess.run(*args, check=check, shell=shell)
    if check and proc.returncode != 0:
        print('command failed: {}: {}', args, proc.returncode)
        sys.exit(proc.returncode)
    return proc

def apt_install(name):
    run(['apt', 'install', name])

def wget(name):
    run('wget', name)

def tar_extract(name, z=False):
    run('tar', '-xzf' if z else '-xf', name)

def link(name, target, symbolic=True):
    args = ['ln']
    if symbolic:
        args.append('-s')
    args.append(target)
    args.append(name)
    run(*args)

def move(src, dest):
    run(['mv', src, dest])

args = get_args()
if args.tuna == 'yes':
    print('Use TUNA package repository')
    run('./scripts/use-tuna.py')
run('apt', 'update')


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
    print(f'Downloading python {version} source code')
    wget(f'https://www.python.org/ftp/python/{version}/Python-{version}.tar.xz')
    tar_extract(f'Python-{version}.tar.xz')
    os.remove(f'Python-{version}.tar.xz')
    move(f'Python-{version}', f'{version}')

    os.chdir(f'{version}')

    print(f'Building python {version} from source')
    run('./configure')
    run('make', f'-j{build_jobs}')

    simp_version = '.'.join(version.split('.')[:2])
    print(f'Installing python {version} to python{simp_version}')
    link(f'/usr/bin/python{simp_version}', './python')

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
        print(f'Installing rust toolchain version {v}')
        run('rustup', 'toolchain', 'install', v)

install_rust('1.38', '1.39', '1.40')

print('Congratulations.')
