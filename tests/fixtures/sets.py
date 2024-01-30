import json
import requests
from tests.fixtures.base import TRACIM_HOST
from tests.fixtures.model import User, Workspace


SETS = {
    "Set1": [
        "/file_2.txt",
        "/folder_1",
        "/folder_1/file_1.txt",
    ],
    "Set2": [
        "/file_toto.txt",
    ],
}

FILE_CONTENTS = {
    "/file_toto.txt": b"toto",
    "/file_2.txt": b"Hello world !",
    "/folder_1/file_1.txt": b"Hello world again !",
}


def create_file(
    container_port: int,
    user: User,
    workspace: Workspace,
    name: str,
    content: bytes,
    parent_id: int = None,
) -> int:
    data = {}
    if parent_id is not None:
        data["parent_id"] = parent_id
    response = requests.post(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces/{workspace.id}/files",
        files={"files": (name, content)},
        data=data,
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    response_json = json.loads(response.content)
    return response_json["content_id"]


def update_file(
    container_port: int,
    user: User,
    workspace: Workspace,
    content_id: int,
    name: str,
    content: bytes,
) -> None:
    response = requests.put(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces/{workspace.id}/files/{content_id}/raw/{name}",
        files={"files": (name, content)},
        auth=(user.username, user.password),
    )
    assert response.status_code == 204


def change_file_workspace(
    container_port: int,
    user: User,
    content_id: int,
    current_workspace_id: int,
    new_workspace_id: int,
) -> None:
    response = requests.put(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces/{current_workspace_id}/contents/{content_id}/move",
        # FIXME BS NOW: determine new_parent_id
        json={"new_parent_id": 0, "new_workspace_id": new_workspace_id},
        auth=(user.username, user.password),
    )
    assert response.status_code == 200


def rename_file(
    container_port: int,
    user: User,
    content_id: int,
    workspace_id: int,
    new_label: str,
) -> None:
    response = requests.put(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces/{workspace_id}/files/{content_id}",
        json={"label": new_label},
        auth=(user.username, user.password),
    )
    assert response.status_code == 200


def create_folder(
    container_port: int,
    user: User,
    workspace: Workspace,
    name: str,
    parent_id: int = None,
) -> int:
    json_ = {"label": name, "content_type": "folder"}
    if parent_id is not None:
        json_["parent_id"] = parent_id
    response = requests.post(
        f"http://{TRACIM_HOST}:{container_port}/api/workspaces/{workspace.id}/contents",
        json=json_,
        auth=(user.username, user.password),
    )
    assert response.status_code == 200
    response_json = json.loads(response.content)
    return response_json["content_id"]


def create_set_on_remote(
    container_port: int, user: User, workspace: Workspace, set_name: str
) -> None:
    content_ids = {}
    for file_path in SETS[set_name]:
        create_remote(
            container_port,
            user,
            workspace,
            file_path,
            content_ids,
            contents=FILE_CONTENTS,
        )


def create_remote(
    container_port: int,
    user: User,
    workspace: Workspace,
    file_path: str,
    content_ids: dict,
    contents: dict,
) -> None:
    # Create only the last part (set must be ordered correctly)
    splitted = file_path[1:].split("/")
    concerned_part = splitted[-1]
    parent_id = None

    if len(splitted) > 1:
        parent_id = content_ids["/" + "/".join(splitted[:-1])]

    if concerned_part.startswith("file_"):
        id = create_file(
            container_port,
            user,
            workspace,
            concerned_part,
            content=contents[file_path],
            parent_id=parent_id,
        )
    elif concerned_part.startswith("folder_"):
        id = create_folder(
            container_port,
            user,
            workspace,
            concerned_part,
            parent_id=parent_id,
        )

    content_ids[file_path] = id
