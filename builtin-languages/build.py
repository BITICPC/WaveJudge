#!/usr/bin/env python3

# This file is expected to be invoked after the `builtin-languages` crate has been built.

import argparse
import pathlib
import subprocess

def get_args():
    parser = argparse.ArgumentParser(description='This script copies configuration and script ' +
        'files into output directory after built.')
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

target_script_dir = pathlib.Path(args.out)
target_script_dir = target_script_dir.joinpath('scripts')
if not target_script_dir.exists():
    target_script_dir.mkdir()

run(f'cp -R ./config/* "{target_config_dir}"')
run(f'cp -R ./scripts/* "{target_script_dir}"')
