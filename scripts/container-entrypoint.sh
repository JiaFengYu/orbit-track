#!/bin/sh
set -eu
exec orbit-track serve --allow-public --listen "0.0.0.0:${PORT:-10000}" "$@"
