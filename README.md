# WaveJudge

Judge node application of WaveJudge system. Written in pure Rust.

## Features

* Secure. We implemented a sandboxed environment to execute anything from compiler to user's program. With the support of `libseccomp` the behavior of sandboxed programs can be controlled.
* Scalable. Easy to add new programming language specifications to the judge.
* Configurable. `WaveJudge` is highly configurable and most of configurations have default values so do not panic :).

## Installation

### System requirements

`WaveJudge` can only be deployed to Linux platforms. It is recommended to use docker to deploy `WaveJudge` which is easy while good enough for most use cases.

### Docker Deployment

> To be finished.

### Manual Deployment

If you have a very good reason to deploy `WaveJudge`, you can do it manually by following instructions in this section.

> To be finished.

## Related Projects

* [BitWaves.JudgeBoard](https://github.com/BITICPC/BitWaves.JudgeBoard)
