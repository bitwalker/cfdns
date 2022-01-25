#!/bin/sh

if podman container exists cfdns; then
    podman start cfdns
else
    podman run -i -d --rm \
        --net=host \
        --name cfdns \
        --security-opt=no-new-privileges \
        -e CONFIG=/etc/cfdns.toml \
        -v /mnt/data/cfdns/config.toml:/etc/cfdns.toml \
        bitwalker/cfdns:latest
fi
