#!/usr/bin/env python3

import sys
import os
import requests
from urllib.request import urlretrieve

REPO = "mozilla-esr140"
HEADERS = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) mozjs-sys/1.0"}


def download_artifact(task_id: str, artifact_name: str, dl_name: str, i=0):
    response = requests.get(
        f"https://firefox-ci-tc.services.mozilla.com/api/queue/v1/task/{task_id}/runs/{i}/artifacts",
    )
    response.raise_for_status()
    artifacts = response.json()["artifacts"]
    if not artifacts:
        if i < 5:
            download_artifact(task_id, artifact_name, dl_name, i + 1)
            return
        else:
            print(f"Error: No artifacts found for task {task_id} after {i} attempts")
            sys.exit(1)
    file = None
    for artifact in artifacts:
        if artifact_name in artifact["name"]:
            file = artifact["name"]
            break

    if file is None:
        print(f"Error: Could not find {artifact_name} artifact")
        sys.exit(1)

    url = f"https://firefox-ci-tc.services.mozilla.com/api/queue/v1/task/{task_id}/runs/{i}/artifacts/{file}"
    print(f"Downloading: {url}")

    urlretrieve(url, dl_name)


def download_from_taskcluster(commit: str):
    response = requests.get(
        f"https://treeherder.mozilla.org/api/project/{REPO}/push/?revision={commit}",
        headers=HEADERS,
    )
    response.raise_for_status()
    job_id = response.json()["results"][0]["id"]
    print(f"Job id {job_id}")

    response = requests.get(
        f"https://treeherder.mozilla.org/api/jobs/?push_id={job_id}",
        headers=HEADERS,
    )
    response.raise_for_status()
    sm_pkg_task_id = None
    hazard_task_id = None
    for result in response.json()["results"]:
        if "spidermonkey-sm-package-linux64/opt" in result:
            sm_pkg_task_id = result[14]
        elif "hazard-linux64-haz/debug" in result:
            hazard_task_id = result[14]

    if sm_pkg_task_id is None:
        print("Error: Could not find spidermonkey-sm-package-linux64/opt task")
        sys.exit(1)
    else:
        print(f"Spidermonkey package task id {sm_pkg_task_id}")
        download_artifact(sm_pkg_task_id, "tar.xz", "mozjs.tar.xz")
    if hazard_task_id is None:
        print("Error: Could not find hazard-linux64-haz/debug task")
        sys.exit(1)
    else:
        print(f"Hazard task id {hazard_task_id}")
        download_artifact(hazard_task_id, "allFunctions.txt.gz", "allFunctions.txt.gz")
        download_artifact(hazard_task_id, "gcFunctions.txt.gz", "gcFunctions.txt.gz")


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    commit_file = os.path.join(script_dir, "COMMIT")
    with open(commit_file, "r") as f:
        commit = f.read().strip()
    print(f"Commit: {commit}")
    download_from_taskcluster(commit)
