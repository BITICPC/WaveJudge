#!/usr/bin/env python3

# This file is expected to be invoked after the `driver` crate has been built.

import argparse
import pathlib
import subprocess

def get_args():
    parser = argparse.ArgumentParser(description='This script copies configuration files into ' +
        'output directory after built.')
    parser.add_argument('-o', '--out', required=True,
        help='The output directory of the build.')
    return parser.parse_args()

def run(args):
    subprocess.run(args, shell=True, check=True)

args = get_args()

target_config_dir = pathlib.Path(args.out)
target_config_dir = target_config_dir.joinpath('config')
if not target_config_dir.exists():
    target_config_dir.mkdir()

run('cp -R ./config/* "{}"'.format(target_config_dir))
