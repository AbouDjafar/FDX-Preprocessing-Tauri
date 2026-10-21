import subprocess

import pytest

from scripts.publish_release import find_release, upload_asset


def test_find_release_includes_drafts():
    draft = {"tag_name": "v1.1.0", "draft": True}
    assert find_release(lambda url: [draft], "https://api.github.com/releases", "v1.1.0") is draft


def test_find_release_paginates():
    urls = []

    def request(url):
        urls.append(url)
        return [{"tag_name": "old"}] * 100 if len(urls) == 1 else [{"tag_name": "v1.1.0"}]

    assert find_release(request, "https://api.github.com/releases", "v1.1.0")["tag_name"] == "v1.1.0"
    assert urls[-1].endswith("page=2")


def test_duplicate_drafts_are_not_automatically_replaced():
    with pytest.raises(ValueError, match="Plusieurs releases"):
        find_release(lambda url: [{"tag_name": "v1"}] * 2, "https://api.github.com/releases", "v1")


def test_upload_credentials_are_not_in_process_arguments(monkeypatch, tmp_path):
    def run(command, **kwargs):
        assert "secret-value" not in str(command)
        assert "Authorization: Bearer secret-value" in kwargs["input"]
        assert "--data-binary" in command
        return subprocess.CompletedProcess(command, 0, '{"size": 42}', "")

    monkeypatch.setattr(subprocess, "run", run)
    result = upload_asset(
        "https://uploads.github.com/example",
        tmp_path / "test.zip",
        {"Authorization": "Bearer secret-value"},
    )
    assert result["size"] == 42


def test_upload_refuses_other_hosts(tmp_path):
    with pytest.raises(ValueError, match="Hôte"):
        upload_asset("https://example.com/", tmp_path / "test.zip", {})
