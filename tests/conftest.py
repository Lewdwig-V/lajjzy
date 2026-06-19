import shutil
import subprocess
from pathlib import Path

import pytest

jj_required = pytest.mark.skipif(
    shutil.which("jj") is None, reason="jj CLI not in PATH"
)


@pytest.fixture
def temp_repo(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    subprocess.run(["jj", "git", "init"], cwd=repo, check=True,
                   capture_output=True)
    (repo / "a.txt").write_text("hello\n")
    subprocess.run(["jj", "describe", "-m", "first change"], cwd=repo,
                   check=True, capture_output=True)
    return repo
