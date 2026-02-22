#!/usr/bin/env python3

import os
import sys
import re
import requests

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from get_taskcluster_mozjs import download_from_taskcluster

REPO = "mozilla-esr140"


def get_latest_mozjs_tag_changeset() -> tuple[str, str]:
    # Obtain latest released tag
    response = requests.get(f"https://hg.mozilla.org/releases/{REPO}/json-tags")
    response.raise_for_status()

    tags = response.json()["tags"]

    matching = []
    for tag_info in tags:
        tag = tag_info["tag"]
        if re.match(r"^FIREFOX_140_.*_RELEASE$", tag):
            matching.append((tag, tag_info["node"]))

    if not matching:
        print("Error: No matching FIREFOX_*_RELEASE tags found")
        sys.exit(1)

    # Sort by version and get the latest
    matching.sort(key=lambda x: x[0])
    tag, changeset = matching[-1]

    return tag, changeset


if __name__ == "__main__":
    tag, changeset = get_latest_mozjs_tag_changeset()
    print(f"Latest tag: {tag}, changeset: {changeset}")

    download_from_taskcluster(changeset)
