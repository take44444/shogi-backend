import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import tempfile
from urllib.parse import quote


ASSETS = Path(__file__).with_suffix("")


def git(root, *arguments):
    try:
        result = subprocess.run(["git", "-C", str(root), *arguments], capture_output=True)
    except FileNotFoundError:
        return None
    return result.stdout if result.returncode == 0 else None


def github_base(origin):
    if origin is None:
        return None
    match = re.fullmatch(r"(?:https://github\.com/|git@github\.com:|ssh://git@github\.com/)([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+?)(?:\.git)?/?", origin.decode("utf-8", errors="replace").strip())
    if match is None or match[1] in (".", "..") or match[2] in (".", ".."):
        return None
    return f"https://github.com/{match[1]}/{match[2]}"


def capture(root, bundle):
    root = root.resolve()
    revision_bytes = git(root, "rev-parse", "--verify", "HEAD")
    revision = revision_bytes.decode().strip() if revision_bytes else None
    base = github_base(git(root, "remote", "get-url", "origin"))
    files = {}

    def source(path):
        if path in files:
            return files[path]
        if path.startswith("@generated/"):
            if path not in bundle.get("generated_sources", {}):
                raise ValueError(f"missing generated source during capture: {path}")
            files[path] = {"content": bundle["generated_sources"][path], "github_url": None,
                           "status": "Generated Rust source captured from the compiler; no repository or GitHub location."}
            return files[path]
        relative = PurePosixPath(path)
        if relative.is_absolute() or ".." in relative.parts or str(relative) != path:
            raise ValueError(f"source path must be repository-relative: {path}")
        resolved = (root / path).resolve()
        if not resolved.is_relative_to(root) or not resolved.is_file():
            raise ValueError(f"source file missing or outside repository: {path}")
        raw = resolved.read_bytes()
        content = raw.decode("utf-8")
        remote = None
        status = "Git のリビジョンがないため、取り込んだローカルの内容を表示します。"
        if revision:
            committed = git(root, "show", f"{revision}:{path}")
            if committed != raw:
                status = "内容が取り込み時の HEAD と異なるか未追跡のため、GitHub リンクはありません。"
            elif base is None:
                status = "内容は取り込み時の HEAD と一致します。対応する GitHub origin がないため、リンクはありません。"
            else:
                remote = f"{base}/blob/{revision}/" + quote(path, safe="/")
                status = "内容は取り込み時の HEAD とバイト単位で一致します。"
        files[path] = {"content": content, "github_url": remote, "status": status}
        return files[path]

    for category in ("apis", "calls", "references", "tests", "unresolved"):
        for record in bundle.get(category, []):
            path, line = record["path"], record["line"]
            lines = source(path)["content"].splitlines()
            if type(line) is not int or not 0 < line <= len(lines):
                raise ValueError(f"invalid compiler source location during capture: {path}:{line}")
    for unit in bundle["units"]:
        source(unit["source"])
    return {"version": 4, "repository": {"revision": revision, "github_base": base},
            "bundle": bundle, "files": files}


def protect_destination(root, destination, payload):
    destination = destination.resolve()
    inputs = {(root / path).resolve() for path in payload["files"] if not path.startswith("@generated/")}
    tool_files = {Path(__file__).resolve(), Path(__file__).with_name("traceability.py").resolve()}
    tool_files.update(path.resolve() for path in ASSETS.rglob("*"))
    tracked = git(root, "ls-files", "-z")
    if tracked is not None:
        inputs.update((root / os.fsdecode(path)).resolve() for path in tracked.split(b"\0") if path)
    metadata = {(root / ".git").resolve()}
    for option in ("--absolute-git-dir", "--git-common-dir"):
        directory = git(root, "rev-parse", option)
        if directory:
            metadata.add((root / os.fsdecode(directory).strip()).resolve())
    source_extensions = {".rs", ".py", ".js", ".css", ".toml", ".md", ".yaml", ".yml", ".proto"}
    if (destination in inputs or destination in tool_files or destination.suffix.lower() in source_extensions
            or destination.is_relative_to(ASSETS.resolve()) or destination.is_relative_to((root / "@generated").resolve())
            or any(destination.is_relative_to(directory) for directory in metadata)):
        raise ValueError(f"refusing to overwrite source, tracked file, or viewer tool: {destination}")
    return destination


def write_viewer(root, bundle, destination):
    root = root.resolve()
    payload = capture(root, bundle)
    destination = protect_destination(root, Path(destination), payload)
    data = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    data = data.replace("&", "\\u0026").replace("<", "\\u003c").replace(">", "\\u003e")
    template = (ASSETS / "index.html").read_text(encoding="utf-8")
    output = template.replace("/* VIEWER_CSS */", (ASSETS / "viewer.css").read_text(encoding="utf-8"))
    output = output.replace("/* VIEWER_JS */", (ASSETS / "viewer.js").read_text(encoding="utf-8"))
    output = output.replace("<!-- VIEWER_DATA -->", data)
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".traceability-", suffix=".html", delete=False) as file:
            temporary = Path(file.name)
            file.write(output.encode("utf-8"))
        os.replace(temporary, destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
