"""Publish verified binaries using Git Credential Manager, never logging credentials."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


def credentials():
    result = subprocess.run(
        ["git", "credential", "fill"],
        input="protocol=https\nhost=github.com\n\n",
        text=True,
        capture_output=True,
        check=True,
    )
    values = dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)
    if not values.get("password"):
        raise RuntimeError("Authentification GitHub nécessaire via Git Credential Manager.")
    return values["password"]


def publish(repo, tag, assets, notes):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("Dépôt invalide")
    if subprocess.check_output(["git", "status", "--porcelain"], text=True).strip():
        raise ValueError("Enregistrer toutes les modifications dans Git avant publication.")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    if not all(path.is_file() for path in assets):
        raise ValueError("Un fichier de distribution est absent.")
    secret = credentials()

    def request(url, method="GET", data=None, binary=False):
        if urllib.parse.urlparse(url).hostname not in {"api.github.com", "uploads.github.com"}:
            raise ValueError("Hôte GitHub inattendu")
        headers = {
            "Authorization": f"Bearer {secret}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2026-03-10",
            "User-Agent": "FDX-Atelier-release",
            "Content-Type": "application/octet-stream" if binary else "application/json",
        }
        payload = data if binary or data is None else json.dumps(data).encode()
        req = urllib.request.Request(url, data=payload, method=method, headers=headers)
        with urllib.request.urlopen(req, timeout=300) as response:
            return json.load(response)

    base = f"https://api.github.com/repos/{repo}/releases"
    try:
        release = request(base + "/tags/" + urllib.parse.quote(tag, safe=""))
    except urllib.error.HTTPError as exc:
        if exc.code != 404:
            raise
        release = request(
            base,
            "POST",
            {
                "tag_name": tag,
                "target_commitish": commit,
                "name": f"FDX Atelier Données {tag}",
                "body": notes.read_text(encoding="utf-8"),
                "draft": True,
                "prerelease": False,
            },
        )
    if not release["draft"]:
        raise ValueError("Cette release est déjà publiée ; aucun contenu n'a été remplacé.")
    existing = {asset["name"]: asset for asset in release["assets"]}
    for path in assets:
        data = path.read_bytes()
        digest = "sha256:" + hashlib.sha256(data).hexdigest()
        if path.name in existing:
            if existing[path.name].get("digest") != digest:
                raise ValueError("Un fichier différent existe dans la release brouillon : " + path.name)
            continue
        upload = release["upload_url"].split("{", 1)[0] + "?name=" + urllib.parse.quote(path.name)
        asset = request(upload, "POST", data, binary=True)
        if asset["size"] != len(data) or (asset.get("digest") and asset["digest"] != digest):
            raise ValueError("Le fichier publié n'a pas la taille ou l'empreinte attendue.")
        print("Fichier envoyé : " + path.name, flush=True)
    release = request(base + "/" + str(release["id"]), "PATCH", {"draft": False})
    print(release["html_url"], flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default="AbouDjafar/FDX-Preprocessing")
    parser.add_argument("--tag", required=True)
    parser.add_argument("--notes", type=Path, required=True)
    parser.add_argument("assets", type=Path, nargs="+")
    args = parser.parse_args()
    try:
        publish(args.repo, args.tag, args.assets, args.notes)
    except urllib.error.HTTPError as exc:
        raise SystemExit(f"Publication GitHub refusée : HTTP {exc.code}") from None
