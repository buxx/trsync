import json
import os
import signal
import subprocess
import time
from pathlib import Path
import typing
import docker
import pytest
import requests
import psutil

from tests.fixtures.model import User, Workspace


TRACIM_VERSION = "4.1.3"
TRACIM_CONTAINER_NAME = "tracim_rsync_test_instance"
TRACIM_HTTP_PORT = 8080
TRACIM_HOST = "localhost"
DEFAULT_LOGIN = "admin@admin.admin"
DEFAULT_PASSWORD = "admin@admin.admin"
USERS = {
    "user1": User("user1", "user1@user1@user1", "user1@user1@user1"),
    "user2": User("user2", "user2@user2@user2", "user2@user2@user2"),
}


def _find_container(name):
    docker_ = docker.from_env()
    containers = docker_.containers.list()
    return next(c for c in containers if c.attrs["Name"] == f"/{name}")


def _ensure_user(user: User, container_port: int):
    # TODO : check before create (or assume correct creation error)
    response = requests.post(
        f"http://{TRACIM_HOST}:{container_port}/api/users",
        json={
            "email": user.email,
            "password": user.password,
            "profile": "administrators",
            "public_name": user.username,
            "username": user.username,
            "email_notification": False,
        },
        auth=(DEFAULT_LOGIN, DEFAULT_PASSWORD),
    )
    assert response.status_code == 200


def stopped_tracim_instance(name):
    try:
        while _find_container(name):
            try:
                _find_container(name).stop()
            except StopIteration:
                pass
    except StopIteration:
        pass


def fresh_tracim_instance(name, port):
    docker_ = docker.from_env()
    docker_.containers.run(
        f"algoo/tracim:{TRACIM_VERSION}",
        detach=True,
        name=name,
        ports={"80/tcp": port},
        auto_remove=True,
        environment=["DATABASE_TYPE=sqlite"],
    )

    # Wait for Tracim http response to consider it as ready
    while True:
        try:
            response = requests.get(f"http://localhost:{port}")
            if response.status_code == 200:
                break
        except requests.exceptions.ConnectionError:
            pass
        time.sleep(0.250)


def ensure_users(container_port):
    _ensure_user(USERS["user1"], container_port)


def create_workspace(container_port: int, owner: User, name: str) -> Workspace:
    response = requests.post(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces",
        json={
            "access_type": "confidential",
            "agenda_enabled": False,
            "default_user_role": "reader",
            "description": "A super description of my workspace.",
            "label": name,
            "public_download_enabled": False,
            "public_upload_enabled": False,
            "publication_enabled": False,
        },
        auth=(owner.username, owner.password),
    )
    assert response.status_code == 200
    response_json = json.loads(response.content)
    id_ = response_json["workspace_id"]
    return Workspace(
        id=id_,
        name=name,
    )


def get_workspace_by_name(container_port: int, user: User, name: str) -> Workspace:
    response = requests.get(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    for workspace_raw in response.json():
        if workspace_raw["label"] == name:
            id_ = workspace_raw["workspace_id"]
            return Workspace(
                id=id_,
                name=name,
            )

    raise Exception(f"Workspace '{name}' not found")


def execute_trsync_and_wait_finished(
    container_port: int,
    folder: Path,
    workspace_id: int,
    user: User,
    stdout,
) -> None:
    args = [
        f"{Path.home()}/.cargo/bin/cargo",
        "run",
        "--bin",
        "trsync",
        str(folder),
        f"{TRACIM_HOST}:{container_port}",
        str(workspace_id),
        user.username,
        "--env-var-pass PASSWORD",
        "--exit-after-sync",
        "--no-ssl",
    ]
    subprocess.run(
        " ".join(args),
        stdout=stdout,
        stderr=stdout,
        env={"PASSWORD": user.password, "RUST_LOG": "DEBUG"},
        shell=True,
        check=True,
    )


@pytest.fixture
def container_port():
    pytest.tracim_http_port_counter = pytest.tracim_http_port_counter + 1
    return TRACIM_HTTP_PORT + pytest.tracim_http_port_counter


@pytest.fixture(autouse=True, scope="module")
def setup(request):
    def end():
        for proc in psutil.process_iter():
            try:
                if "target/debug/trsync /tmp/pytest-of" in proc.name().lower():
                    os.kill(proc.pid, signal.SIGKILL)

            except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
                pass

    request.addfinalizer(end)


def execute_trsync(
    container_port: int,
    folder: Path,
    workspace_id: int,
    user: User,
    stdout,
) -> int:
    cargo_bin_path = f"{Path.home()}/.cargo/bin/cargo"
    args = [
        cargo_bin_path,
        "run",
        "--bin",
        "trsync",
        str(folder),
        f"{TRACIM_HOST}:{container_port}",
        str(workspace_id),
        user.username,
        "--env-var-pass PASSWORD",
        "--no-ssl",
    ]
    log_level = os.environ.get("RUST_LOG", "DEBUG")
    subprocess.Popen(
        " ".join(args),
        stdout=stdout,
        stderr=stdout,
        env={"PASSWORD": user.password, "RUST_LOG": log_level},
        shell=True,
    )

    # Search trsync pid launch through shell
    pids = []
    for pid in psutil.pids():
        process = psutil.Process(pid)
        cmdline = " ".join(process.cmdline())
        if cargo_bin_path in cmdline and str(folder) in cmdline:
            pids.append(pid)

    assert pids, "Programmatic error : trsync pid not found"
    return pids


def get_folder_listing(path: Path) -> typing.List[str]:
    paths = []
    for p in path.glob("**/*"):
        if ".trsync.db" in p.name:
            continue
        paths.append(str(p).replace(str(path), ""))
    return list(sorted(paths))


def _get_workspace_contents(
    container_port: int,
    user: User,
    workspace: Workspace,
) -> typing.List[dict]:
    response = requests.get(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces/{workspace.id}/contents",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    return json.loads(response.content)["items"]


def get_content(container_port: int, user: User, content_id: int) -> dict:
    response = requests.get(
        f"http://{TRACIM_HOST}:{container_port}/api/contents/{content_id}",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    return json.loads(response.content)


def get_content_bytes(container_port: int, user: User, content_id: int) -> bytes:
    response = requests.get(
        f"http://{TRACIM_HOST}:{container_port}/api/contents/{content_id}",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    content = json.loads(response.content)

    workspace_id = content["workspace_id"]
    filename = content["filename"]
    response = requests.get(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces/{workspace_id}/files/{content_id}/raw/{filename}",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    return response.content


def _get_content_path(container_port: int, user: User, content_id: int) -> Path:
    paths = []
    current_content_id = content_id

    while True:
        parent = get_content(container_port, user, current_content_id)
        if parent["is_deleted"]:
            raise IndexError()
        paths.append(parent["filename"])
        if not parent["parent_id"]:
            break
        current_content_id = parent["parent_id"]

    return Path().joinpath(*reversed(paths))


def get_workspace_listing(
    container_port: int,
    user: User,
    workspace: Workspace,
) -> typing.Dict[str, int]:
    paths = {}

    for content in _get_workspace_contents(container_port, user, workspace):
        try:
            if parent_id := content["parent_id"]:
                parent_path = _get_content_path(container_port, user, parent_id)
                path = (parent_path / Path(content["filename"])), content["content_id"]
            else:
                path = Path(content["filename"]), content["content_id"]
        except IndexError:
            pass
        paths[str("/" / path[0])] = path[1]

    return paths


def check_until(callback, duration=10.0):
    start = time.time()
    while True:
        try:
            callback()
            return
        except AssertionError as exc:
            if time.time() - start > duration:
                raise exc
            time.sleep(0.250)


@pytest.fixture(autouse=True, scope="function")
def setup(request):
    def stop_container():
        container_name = f"{TRACIM_CONTAINER_NAME}-{request.node.name}"
        stopped_tracim_instance(container_name)

    def stop_trsync():
        for trsync_pid in next(
            prop for prop in request.node.user_properties if prop[0] == "trsync_pid"
        )[1]:
            trsync_process = psutil.Process(trsync_pid)
            trsync_process.kill()
            trsync_process.wait()

    request.addfinalizer(stop_container)
    request.addfinalizer(stop_trsync)


@pytest.fixture(scope="function")
def content_ids() -> typing.Dict[str, int]:
    return {}
