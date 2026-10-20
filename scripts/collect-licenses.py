"""Bundle dependency license files; never reads the user's application data."""
import json
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "resources" / "licenses"
OUT.mkdir(parents=True, exist_ok=True)
data = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version=1"], cwd=ROOT))
index = []
for package in data["packages"]:
    base = pathlib.Path(package["manifest_path"]).parent
    if "registry" not in str(base):
        continue
    name = f"{package['name']}-{package['version']}"
    index.append({"name": name, "license": package["license"], "repository": package.get("repository")})
    for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*"):
        for source in base.glob(pattern):
            if source.is_file():
                dest = OUT / name / source.name
                dest.parent.mkdir(exist_ok=True)
                shutil.copyfile(source, dest)
for package in ("react", "react-dom", "scheduler", "lucide-react", "@tauri-apps/api", "@tauri-apps/plugin-dialog"):
    base = ROOT / "node_modules" / package
    if not base.exists():
        continue
    manifest = json.loads((base / "package.json").read_text(encoding="utf-8"))
    name = package.replace("/", "_").replace("@", "") + "-" + manifest["version"]
    index.append({"name": name, "license": manifest.get("license")})
    for source in base.glob("*"):
        if source.is_file() and source.name.upper().startswith(("LICENSE", "LICENCE", "NOTICE")):
            dest = OUT / name / source.name
            dest.parent.mkdir(exist_ok=True)
            shutil.copyfile(source, dest)
(OUT / "index.json").write_text(json.dumps(index, indent=2), encoding="utf-8")
print(f"Collected notices for {len(index)} dependencies")
