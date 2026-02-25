#!/usr/bin/env python3

import os
import sys
import re
import requests

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from get_taskcluster_mozjs import download_from_taskcluster, ESR, REPO


def get_latest_mozjs_tag_changeset() -> tuple[str, str, str]:
    # Obtain latest released tag
    response = requests.get(f"https://hg.mozilla.org/releases/{REPO}/json-tags")
    response.raise_for_status()

    tags = response.json()["tags"]

    matching = []
    for tag_info in tags:
        tag = tag_info["tag"]
        if re.match(rf"^FIREFOX_{ESR}_.*esr_RELEASE$", tag):
            matching.append(
                (
                    float(
                        tag.removeprefix(f"FIREFOX_{ESR}_")
                        .removesuffix("esr_RELEASE")
                        .replace("_", ".")
                    ),
                    tag,
                    tag_info["node"],
                )
            )

    if not matching:
        print("Error: No matching FIREFOX_*_RELEASE tags found")
        sys.exit(1)

    # Sort by version and get the latest
    matching.sort(key=lambda x: x[0])
    minor_patch, tag, changeset = matching[-1]

    return minor_patch, tag, changeset


if __name__ == "__main__":
    minor_patch, tag, changeset = get_latest_mozjs_tag_changeset()
    print(f"Latest tag: {tag}, changeset: {changeset}")

    download_from_taskcluster(changeset)
