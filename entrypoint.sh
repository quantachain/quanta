#!/bin/bash
set -e

# Ensure data directories exist and have correct permissions
mkdir -p /home/quanta/quanta_data_node1
mkdir -p /home/quanta/quanta_data_node2
mkdir -p /home/quanta/quanta_data_node3

# Fix permissions if running as root (shouldn't happen, but just in case)
if [ "$(id -u)" = "0" ]; then
    chown -R quanta:quanta /home/quanta
    exec su-exec quanta "$@"
fi

# Execute the command
exec "$@"
