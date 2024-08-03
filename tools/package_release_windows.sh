#!/usr/bin/env bash
set -e  # stop at first error

[[ -z "$1" ]] && { echo "Please give package reference name as first parameter" ; exit 1; }
FOLDER_NAME=TrSync_${1}_Win64

mkdir -p ${FOLDER_NAME}
cp target/release/trsync.exe ${FOLDER_NAME}
cp target/release/trsync_manager.exe ${FOLDER_NAME}
cp target/release/trsync_manager_systray.exe ${FOLDER_NAME}
cp LICENSE ${FOLDER_NAME}
zip -r ${FOLDER_NAME}.zip ${FOLDER_NAME}
