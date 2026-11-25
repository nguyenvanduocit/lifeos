"""Forward GitHub inbox thread updates to the local LifeOS signed webhook."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import uuid


def timestamp(time):
    return time.strftime("%Y-%m-%dT%H:%M:%SZ")


def main():
    home = Path(os.environ.get("LIFEOS_HOME", Path.home() / "lifeos")) / ".lifeos"
    config = json.loads((home / "plugins.json").read_text()).get("github", {})
    if not config or config.get("disabled"):
        return
    if hashlib.sha256(Path(config["binary"]).read_bytes()).hexdigest() != config["sha256"]:
        raise RuntimeError("GitHub package fingerprint changed")
    cursor = home / "github-notifications.cursor"
    now = datetime.datetime.now(datetime.timezone.utc)
    # Overlap the boundary. Stable delivery IDs make replay safe after partial failure.
    checkpoint = timestamp(now - datetime.timedelta(minutes=1))
    if cursor.exists():
        endpoint = "notifications?all=true&per_page=50&since=" + cursor.read_text()
        options = ["--paginate", "--slurp"]
    else:
        endpoint = "notifications?per_page=1"
        options = []
    result = subprocess.check_output(["gh", "api", "--hostname", "github.com", *options, endpoint], timeout=180)
    pages = json.loads(result)
    for page in (pages if options else [pages]):
        for thread in page:
            body = json.dumps(thread).encode()
            identity = f"github.com/notifications/{thread['id']}/{thread['updated_at']}"
            headers = {
                "Content-Type": "application/json",
                "X-GitHub-Event": "notification",
                "X-GitHub-Delivery": str(uuid.uuid5(uuid.NAMESPACE_URL, identity)),
                "X-Hub-Signature-256": subprocess.check_output([config["binary"], "--sign-notification"], input=body, timeout=15).decode().strip(),
            }
            request = urllib.request.Request("http://127.0.0.1:7420/api/plugins/github/hooks/webhook", data=body, headers=headers)
            with urllib.request.urlopen(request, timeout=20) as response:
                response.read()
    staged = cursor.with_suffix(".tmp")
    staged.write_text(checkpoint)
    staged.replace(cursor)


if __name__ == "__main__":
    main()
