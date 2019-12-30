#!/usr/bin/env python3

import argparse
import subprocess
import sys
import os
import pathlib

def parse_args():
    parser = argparse.ArgumentParser(description='WaveJudge build script')
    parser.add_argument('--profile', type=str, default='release', required=True,
        choices=['release', 'debug'],
        help='Profile to use when invoking cargo')

    return parser.parse_args()

def run(*args, shell=True, check=True):
    subprocess.run(*args, shell=shell=, check=check)

def cargo_build(release=False):
    cargo_args = ['cargo', 'build']
    if release:
        cargo_args.append('--release')
    run(*cargo_args)

def subdir_build(subdir, out_dir):
    os.chdir(f'./{subdir}')
    run(f'./{subdir}/build.py', '-o', out_dir)
    os.chdir('..')

args = parse_args()

release = args.profile == 'release'
cargo_build(release=release)

out_dir = pathlib.Path(f'./target/{profile}')
subdirs = ['builtin-languages', 'driver']
for subdir in subdirs:
    subdir_build(subdir, str(out_dir.absolute()))
