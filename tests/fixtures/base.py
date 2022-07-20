import json
import os
from pickle import DEFAULT_PROTOCOL
import signal
import subprocess
import time
from pathlib import Path
import typing
from unicodedata import name
import docker
import pytest
import requests
import psutil

from tests.fixtures.model import Content, User, Workspace


TRACIM_VERSION = "4.1.3"
TRACIM_CONTAINER_NAME = "tracim_rsync_test_instance"
TRACIM_HTTP_PORT = 8080
TRACIM_URL = f"localhost:{TRACIM_HTTP_PORT}"
DEFAULT_LOGIN = "admin@admin.admin"
DEFAULT_PASSWORD = "admin@admin.admin"
USERS = {
    "user1": User("user1", "user1@user1@user1", "user1@user1@user1"),
    "user2": User("user2", "user2@user2@user2", "user2@user2@user2"),
}


def _find_container():
    docker_ = docker.from_env()
    containers = docker_.containers.list()
    return next(c for c in containers if c.attrs["Name"] == f"/{TRACIM_CONTAINER_NAME}")


def _ensure_user(user: User):
    # TODO : check before create (or assume correct creation error)
    response = requests.post(
        f"http://{TRACIM_URL}/api/users",
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


def stopped_tracim_instance():
    try:
        _find_container().stop()
    except StopIteration:
        pass


def fresh_tracim_instance():
    docker_ = docker.from_env()
    docker_.containers.run(
        f"algoo/tracim:{TRACIM_VERSION}",
        detach=True,
        name=TRACIM_CONTAINER_NAME,
        ports={"80/tcp": TRACIM_HTTP_PORT},
        auto_remove=True,
        environment=["DATABASE_TYPE=sqlite"],
    )

    # Wait for Tracim http response to consider it as ready
    while True:
        try:
            response = requests.get(f"http://localhost:{TRACIM_HTTP_PORT}")
            if response.status_code == 200:
                break
        except requests.exceptions.ConnectionError:
            pass
        time.sleep(0.250)


def ensure_users():
    _ensure_user(USERS["user1"])


def create_workspace(owner: User, name: str) -> Workspace:
    response = requests.post(
        f"http://{TRACIM_URL}/api/workspaces",
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


def execute_trsync_and_wait_finished(
    folder: Path, workspace_id: int, user: User, stdout
) -> None:
    args = [
        f"{Path.home()}/.cargo/bin/cargo",
        "run",
        "--bin",
        "trsync",
        str(folder),
        TRACIM_URL,
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


def execute_trsync(folder: Path, workspace_id: int, user: User, stdout):
    args = [
        f"{Path.home()}/.cargo/bin/cargo",
        "run",
        "--bin",
        "trsync",
        str(folder),
        TRACIM_URL,
        str(workspace_id),
        user.username,
        "--env-var-pass PASSWORD",
        "--no-ssl",
    ]
    subprocess.Popen(
        " ".join(args),
        stdout=stdout,
        stderr=stdout,
        env={"PASSWORD": user.password, "RUST_LOG": "DEBUG"},
        shell=True,
    )


def get_folder_listing(path: Path) -> typing.List[str]:
    paths = []
    for p in path.glob("**/*"):
        if ".trsync.db" in p.name:
            continue
        paths.append(str(p).replace(str(path), ""))
    return list(sorted(paths))


def _get_workspace_contents(user: User, workspace: Workspace) -> typing.List[dict]:
    response = requests.get(
        f"http://{TRACIM_URL}/api/workspaces/{workspace.id}/contents",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    return json.loads(response.content)["items"]


def get_content(user: User, content_id: int) -> dict:
    response = requests.get(
        f"http://{TRACIM_URL}/api/contents/{content_id}",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    return json.loads(response.content)


def get_content_bytes(user: User, content_id: int) -> bytes:
    response = requests.get(
        f"http://{TRACIM_URL}/api/contents/{content_id}",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    content = json.loads(response.content)

    workspace_id = content["workspace_id"]
    filename = content["filename"]
    response = requests.get(
        f"http://{TRACIM_URL}/api/workspaces/{workspace_id}/files/{content_id}/raw/{filename}",
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    return response.content


def _get_content_path(user: User, content_id: int) -> Path:
    paths = []
    current_content_id = content_id

    while True:
        parent = get_content(user, current_content_id)
        if parent["is_deleted"]:
            raise IndexError()
        paths.append(parent["filename"])
        if not parent["parent_id"]:
            break
        current_content_id = parent["parent_id"]

    return Path().joinpath(*reversed(paths))


def get_workspace_listing(user: User, workspace: Workspace) -> typing.Dict[str, int]:
    paths = {}

    for content in _get_workspace_contents(user, workspace):
        try:
            if parent_id := content["parent_id"]:
                parent_path = _get_content_path(user, parent_id)
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
