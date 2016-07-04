# Introduction

OMD-Transfer is a tool to wirelessly transfer images from a Olympus
Image Share compatible camera to the computer. It supports incremental
download (transfer everything new) and transferring a predefined list
of pictures marked on the camera (Transfer Order).

# Usage

`omd-transfer` is configured via a config file named `config.toml`. By
default, it searches for a file with that name in the current
directory, or in the path specified either via the
`OMD_TRANSFER_CONFIG` environment variable or the `--config` command
line argument.

You can pass `--write-template` to generate a config template in the
current directory:

```
omd-transfer --write-template
```
