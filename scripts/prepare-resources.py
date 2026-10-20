"""Fetch pinned, checksum-verified OCR assets. Python is a build dependency only."""
import hashlib
import io
import json
import pathlib
import shutil
import urllib.request
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
RESOURCE = ROOT / "src-tauri" / "resources"
MANIFEST = ROOT / "scripts" / "resources.lock.json"


def download(url, checksum):
    request = urllib.request.Request(url, headers={"User-Agent": "InterviewAssistant-Build/0.1"})
    with urllib.request.urlopen(request, timeout=120) as response:
        data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != checksum:
        raise RuntimeError(f"Checksum mismatch: {url}; expected {checksum}, received {digest}")
    return data


def main():
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    for entry in manifest["files"]:
        target = RESOURCE / entry["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.is_file() and hashlib.sha256(target.read_bytes()).hexdigest() == entry["sha256"]:
            print(f"Verified {entry['path']}")
            continue
        data = download(entry["url"], entry["sha256"])
        target.write_bytes(data)
        print(f"Prepared {entry['path']}")
    # paddle-ocr-rs' explicit dictionary API expects CTC blank at index zero
    # and the extra space token at the end; upstream Paddle dictionary omits both.
    raw = (RESOURCE / "models" / "paddle-keys.txt").read_text(encoding="utf-8").rstrip("\r\n")
    (RESOURCE / "models" / "keys.txt").write_text("#\n" + raw + "\n ", encoding="utf-8")
    archive = manifest["runtime"]
    path = ROOT / "artifacts" / archive["filename"]
    path.parent.mkdir(exist_ok=True)
    if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != archive["sha256"]:
        path.write_bytes(download(archive["url"], archive["sha256"]))
    runtime = RESOURCE / "runtime"
    runtime.mkdir(exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(path.read_bytes())) as zipped:
        for info in zipped.infolist():
            basename = pathlib.PurePosixPath(info.filename).name
            if basename in ("onnxruntime.dll", "onnxruntime_providers_shared.dll", "LICENSE", "ThirdPartyNotices.txt"):
                with zipped.open(info) as source, (runtime / basename).open("wb") as destination:
                    shutil.copyfileobj(source, destination)
    assert (runtime / "onnxruntime.dll").is_file()
    (RESOURCE / "resource-manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
