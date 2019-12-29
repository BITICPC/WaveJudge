#!/usr/bin/env python3

import sys
import pathlib
import subprocess

# Usage: java-compiler.py

class Args():
    def __init__(self):
        self.output_dir = None
        self.output_file = None
        self.javac_argv = ['javac']

def get_args():
    argv = sys.argv
    args = Args()

    i = 0
    while i < len(argv):
        if argv[i] == '-o':
            args.output_file = pathlib.Path(argv[i + 1])
            i += 1
        else:
            args.javac_argv.append(argv[i])
            if argv[i] == '-d':
                args.output_dir = pathlib.Path(argv[i + 1])
        i += 1

    return args

args = get_args()
if args.output_dir == None:
    print('No output directory specified.', file=sys.stderr)
    sys.exit(-1)
if args.output_file == None:
    print('No output JAR file specified.', file=sys.stderr)
    sys.exit(-1)

# Invoke javac.
proc = subprocess.run(args.javac_argv)
if proc.returncode != 0:
    sys.exit(proc.returncode)

# List all generated *.class file under output directory.
class_files = list(map(str, args.output_dir.glob('*.class')))
jar_file = str(output_dir.joinpath(args.output_file))
jar_argv = ['jar', '--create', '--file', jar_file] + class_files

# Invoke jar.
proc = subprocess.run(jar_argv)
sys.exit(proc.returncode)
