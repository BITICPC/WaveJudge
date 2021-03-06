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

def run(args):
    subprocess.run(args, shell=True, check=True)

def cargo_build(release=False):
    if release:
        run('cargo build --release')
    else:
        run('cargo build')

def subdir_build(subdir, out_dir):
    os.chdir('./{}'.format(subdir))
    run('./build.py -o "{}"'.format(out_dir))
    os.chdir('..')

args = parse_args()

profile = args.profile
release = profile == 'release'
cargo_build(release=release)

out_dir = pathlib.Path('./target/{}'.format(profile)).resolve()
subdirs = ['builtin-languages', 'driver']
for subdir in subdirs:
    subdir_build(subdir, str(out_dir))
