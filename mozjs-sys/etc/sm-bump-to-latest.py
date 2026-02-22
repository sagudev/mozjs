#!/usr/bin/env python3

import os
import sys
import subprocess

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from get_latest_mozjs import get_latest_mozjs_tag_changeset
from get_taskcluster_mozjs import download_from_taskcluster
from get_mozjs import download_gh_artifact
from update import main

REPO = "mozilla-esr140"

tag, changeset = get_latest_mozjs_tag_changeset()
print(f"Latest tag: {tag}, changeset: {changeset}")

download_from_taskcluster(changeset)

subprocess.check_call(
    [
        "gh",
        "release",
        "create",
        f"mozjs-source-{changeset}",
        "mozjs.tar.xz",
        "allFunctions.txt.gz",
        "gcFunctions.txt.gz",
        "--repo",
        "servo/mozjs",
        "--title",
        f"SpiderMonkey {tag}",
        "--notes",
        f"Source code for SpiderMonkey {tag} (changeset: [{changeset}](https://hg.mozilla.org/releases/{REPO}/rev/{changeset}))",
    ]
)

os.remove("mozjs.tar.xz")
os.remove("allFunctions.txt.gz")
os.remove("gcFunctions.txt.gz")

script_dir = os.path.dirname(os.path.abspath(__file__))
commit_file = os.path.join(script_dir, "COMMIT")
with open(commit_file, "w") as f:
    f.write(changeset)

subprocess.check_call(["git", "add", f"{commit_file}"])
subprocess.check_call(["git", "commit", "-m", "Update COMMIT", "--signoff"])

download_gh_artifact("mozjs.tar.xz")

main(["mozjs.tar.xz"])

os.remove("mozjs.tar.xz")

subprocess.check_call(["git", "add", "--all"])
subprocess.check_call(["git", "commit", "-m", "Apply patches", "--signoff"])
