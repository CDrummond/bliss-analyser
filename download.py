#!/usr/bin/env python3

#
# LMS-BlissMixer
#
# Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
# MIT license.
#

import datetime, os, requests, shutil, subprocess, sys, tempfile, time

GITHUB_TOKEN_FILE = "%s/.config/github-token" % os.path.expanduser('~')
GITHUB_REPO = "CDrummond/bliss-analyser"
LINUX_ARM_ARTIFACTS = ["bliss-analyser-linux-arm-ffmpeg", "bliss-analyser-linux-arm-static", "bliss-analyser-debian-bullseye-arm-libav", "bliss-analyser-debian-bookworm-arm-libav"]
LINUX_X86_ARTIFACTS = ["bliss-analyser-linux-x86-ffmpeg", "bliss-analyser-linux-x86-static", "bliss-analyser-ubuntu-22.04-x86-libav", "bliss-analyser-ubuntu-24.04-x86-libav"]
MAC_ARTIFACTS =       ["bliss-analyser-mac-ffmpeg"]
WINDOWS_ARTIFACTS =   ["bliss-analyser-windows-libav"]
UNIX_ARTIFACTS = LINUX_ARM_ARTIFACTS + LINUX_X86_ARTIFACTS + MAC_ARTIFACTS
GITHUB_ARTIFACTS = UNIX_ARTIFACTS + WINDOWS_ARTIFACTS


def info(s):
    print("INFO: %s" %s)


def error(s):
    print("ERROR: %s" % s)
    exit(-1)


def usage():
    print("Usage: %s <major>.<minor>.<patch>" % sys.argv[0])
    exit(-1)


def to_time(tstr):
    return time.mktime(datetime.datetime.strptime(tstr, "%Y-%m-%dT%H:%M:%SZ").timetuple())


def get_items(repo, artifacts):
    info("Getting artifact list")
    js = requests.get("https://api.github.com/repos/%s/actions/artifacts" % repo).json()
    if js is None or not "artifacts" in js:
        error("Failed to list artifacts")

    items={}
    for a in js["artifacts"]:
        if a["name"] in artifacts and (not a["name"] in items or to_time(a["created_at"])>items[a["name"]]["date"]):
            items[a["name"]]={"date":to_time(a["created_at"]), "url":a["archive_download_url"]}

    return items


def download_artifacts(version):
    items = get_items(GITHUB_REPO, GITHUB_ARTIFACTS)
    if len(items)!=len(GITHUB_ARTIFACTS):
        error("Failed to determine all artifacts")
    token = None
    with open(GITHUB_TOKEN_FILE, "r") as f:
        token = f.readlines()[0].strip()
    headers = {"Authorization": "token %s" % token};

    for item in items:
        dest = "%s-%s.zip" % (item, version)
        info("Downloading %s" % item)
        r = requests.get(items[item]['url'], headers=headers, stream=True)
        with open(dest, 'wb') as f:
            for chunk in r.iter_content(chunk_size=1024*1024): 
                if chunk:
                    f.write(chunk)
        if not os.path.exists(dest):
            info("Failed to download %s" % item)
            break


def make_executable(version):
    cwd = os.getcwd()
    for a in UNIX_ARTIFACTS:
        archive = "%s-%s.zip" % (a, version)
        info("Making analyser executable in %s" % archive)
        with tempfile.TemporaryDirectory() as td:
            subprocess.call(["unzip", archive, "-d", td], shell=False)
            os.remove(archive)
            os.chdir(td)
            subprocess.call(["chmod", "a+x", "%s/bliss-analyser" % td], shell=False)
            bindir = os.path.join(td, "bin")
            if os.path.isdir(bindir):
                for e in os.listdir(bindir):
                    subprocess.call(["chmod", "a+x", os.path.join(bindir, e)], shell=False)
            shutil.make_archive("%s/%s-%s" % (cwd, a, version), "zip")
            os.chdir(cwd)


def checkVersion(version):
    try:
        parts=version.split('.')
        major=int(parts[0])
        minor=int(parts[1])
        patch=int(parts[2])
    except:
        error("Invalid version number")


if 1==len(sys.argv):
    usage()

version=sys.argv[1]
if version!="test":
    checkVersion(version)

download_artifacts(version)
make_executable(version)
