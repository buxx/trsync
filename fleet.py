#!/usr/bin/env python3
import time
import argparse
import configparser
import getpass
import signal
import os
import subprocess

"""
Start multiple trsync instances.
"""


class Workspace:
    def __init__(self, name, folder_path, remote_id):
        self.name = name
        self.folder_path = folder_path
        self.remote_id = remote_id


class Instance:
    def __init__(self, name, domain, username, password, workspaces):
        self.name = name
        self.domain = domain
        self.username = username
        self.password = password
        self.workspaces = workspaces


class Config:
    def __init__(self, bin, log_to, passwords, instances) -> None:
        self.bin = bin
        self.log_to = log_to
        self.passwords = passwords
        self.instances = instances


def clean_from_str_list(value):
    return list(
        filter(
            bool,
            map(lambda i: i.strip(), value.split(",")),
        )
    )


def main(config_file_path):
    # Read ini config file
    config_parser = configparser.ConfigParser()
    config_parser.read(config_file_path)

    # Extract passwords and instance names
    ask_password_ids = clean_from_str_list(config_parser["main"]["ask_password_ids"])
    instances_names = clean_from_str_list(config_parser["main"]["instance_names"])
    assert instances_names, "You must provide instance name(s)"

    # Ask passwords to user
    passwords = {}
    for ask_password_id in ask_password_ids:
        password = getpass.getpass(f"Enter password for '{ask_password_id}': ")
        passwords[ask_password_id] = password

    instances = []
    for instance_name in instances_names:
        domain = config_parser[instance_name]["domain"]
        username = config_parser[instance_name]["username"]
        password_id = config_parser[instance_name]["password_id"]
        assert password_id in passwords, f"Password id '{password_id}' is unknown"
        password = passwords[password_id]
        workspace_names = clean_from_str_list(
            config_parser[instance_name]["workspace_names"]
        )

        workspaces = []
        for workspace_name in workspace_names:
            section_name = f"{instance_name}::{workspace_name}"
            assert section_name in config_parser, f"Section '{section_name}' not found"
            folder_path = config_parser[section_name]["folder_path"]
            remote_id = config_parser[section_name]["remote_id"]
            workspaces.append(Workspace(workspace_name, folder_path, remote_id))

        instances.append(
            Instance(instance_name, domain, username, password, workspaces)
        )

    bin = config_parser["main"]["bin"]
    log_to = config_parser["main"]["log_to"]
    config = Config(bin, log_to, passwords, instances)
    run(config)


def run(config):
    processes = []
    log_files = []

    def _stop(signum, frame):
        print(f"Stop required ({signum}) ...")
        for process in processes:
            process.terminate()
            process.wait()
        exit()

    signal.signal(signal.SIGINT, _stop)
    signal.signal(signal.SIGQUIT, _stop)
    signal.signal(signal.SIGTERM, _stop)

    for instance in config.instances:
        for workspace in instance.workspaces:
            args = [
                config.bin,
                workspace.folder_path,
                instance.domain,
                workspace.remote_id,
                instance.username,
                "--env-var-pass",
                "TRSYNC_PASSWORD",
            ]
            log_file_path = config.log_to.format(
                instance_name=instance.name, workspace_name=workspace.name
            )
            print(
                f"Start sync for : {instance.name}::{workspace.name} "
                f"(\"{' '.join(args)}\") and log into {log_file_path}"
            )
            log_file = open(
                log_file_path,
                "a+",
            )
            log_files.append(log_file)
            process_env = os.environ.copy()
            process_env["TRSYNC_PASSWORD"] = instance.password
            process = subprocess.Popen(
                args,
                stdout=log_file,
                stderr=log_file,
                env=process_env,
            )
            processes.append(process)

    while True:
        time.sleep(5)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Start multiple trsync to sync multiple folders"
    )
    parser.add_argument("config_file_path", type=str)

    args = parser.parse_args()
    main(args.config_file_path)

    # TODO : intercepter le CTRL+C pour arrêter les process
    # TODO : ajouter le flag debug (env var RUST_LOG)
    # TODO : consulter les process regulierement pour voir si ils sont en vie et print si pas le cas
