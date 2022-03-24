#!/bin/bash
set -e

docker login
docker pull algoo/tracim:4.1.3

# TODO : Ensure users