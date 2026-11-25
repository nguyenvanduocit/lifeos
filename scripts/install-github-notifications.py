"""Install the GitHub notification collector for the current macOS user."""
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys

home = Path.home()
lifeos = Path(os.environ.get("LIFEOS_HOME", home / "lifeos")).resolve()
collector = lifeos / ".lifeos" / "github-notifications.py"
shutil.copyfile(Path(__file__).with_name(collector.name), collector)
label = "com.lifeos.github-notifications"
domain = f"gui/{os.getuid()}"
plist = home / "Library" / "LaunchAgents" / (label + ".plist")
gh = shutil.which("gh")
if not gh:
    sys.exit("Install and authenticate GitHub CLI first")
plist.write_bytes(plistlib.dumps({
    "Label": label,
    "ProgramArguments": [sys.executable, str(collector)],
    "StartInterval": 300,
    "RunAtLoad": True,
    "EnvironmentVariables": {"LIFEOS_HOME": str(lifeos), "PATH": str(Path(gh).parent) + ":/usr/bin:/bin"},
    "StandardErrorPath": str(home / "Library" / "Logs" / (label + ".log")),
}))
if subprocess.run(["launchctl", "print", f"{domain}/{label}"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
    subprocess.run(["launchctl", "bootout", f"{domain}/{label}"], check=True)
subprocess.run(["launchctl", "bootstrap", domain, str(plist)], check=True)
