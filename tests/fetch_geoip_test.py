import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/fetch-geoip.sh"
DATABASE = b"current database fixture"
PREVIOUS_DATABASE = b"previous database fixture"
TOKEN = "test-token&with+punctuation"

MOCK_CURL = '''#!/usr/bin/env python3
import hashlib
import json
import os
from pathlib import Path
import sys

arguments = sys.argv[1:]
token = os.environ["IPINFO_DOWNLOAD_TOKEN"]
assert "--get" in arguments
assert arguments[arguments.index("--data-urlencode") + 1] == "token@-"
assert all(token not in argument for argument in arguments)
assert sys.stdin.read() == token

url = next(argument for argument in arguments if argument.startswith("https://"))
base_url = "https://ipinfo.io/data/ipinfo_lite.mmdb"
assert url in (base_url, base_url + "/checksums")

requests = Path(os.environ["RATE_TEST_REQUESTS"])
history = requests.read_text().splitlines()
database = b"current database fixture"
if url.endswith("/checksums"):
    request = "checksum"
    response = json.dumps({"checksums": {"sha256": hashlib.sha256(database).hexdigest()}}).encode()
else:
    request = "database"
    scenario = os.environ["RATE_TEST_SCENARIO"]
    corrupt = scenario == "corrupt" or (scenario == "rotate" and "database" not in history)
    response = b"invalid database fixture" if corrupt else database

with requests.open("a") as log:
    log.write(request + "\\n")
if "--output" in arguments:
    Path(arguments[arguments.index("--output") + 1]).write_bytes(response)
else:
    assert request == "checksum"
    sys.stdout.buffer.write(response)
'''


class FetchGeoIPTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="rate-geoip-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.destination = self.root / "build inputs" / "ipinfo_lite.mmdb"
        self.requests = self.root / "requests.log"
        self.requests.touch()
        curl = self.root / "curl"
        curl.write_text(MOCK_CURL)
        curl.chmod(0o755)

    def fetch(self, scenario, token=TOKEN):
        env = dict(os.environ)
        env.update(
            PATH=f"{self.root}{os.pathsep}{env['PATH']}",
            RATE_TEST_REQUESTS=str(self.requests),
            RATE_TEST_SCENARIO=scenario,
        )
        if token is None:
            env.pop("IPINFO_DOWNLOAD_TOKEN", None)
        else:
            env["IPINFO_DOWNLOAD_TOKEN"] = token
        return subprocess.run(
            ["bash", str(SCRIPT), str(self.destination)],
            cwd=self.root,
            env=env,
            capture_output=True,
            text=True,
            timeout=20,
        )

    def assert_attempts(self, count):
        requests = self.requests.read_text().splitlines()
        self.assertEqual(len(requests), count * 2)
        for offset in range(0, len(requests), 2):
            self.assertCountEqual(requests[offset : offset + 2], ["checksum", "database"])

    def test_downloads_and_verifies_the_latest_database(self):
        result = self.fetch("success")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.destination.read_bytes(), DATABASE)
        self.assert_attempts(1)

    def test_retries_with_a_fresh_pair_after_a_checksum_mismatch(self):
        result = self.fetch("rotate")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.destination.read_bytes(), DATABASE)
        self.assert_attempts(2)

    def test_rejects_corrupt_downloads_without_replacing_the_previous_database(self):
        self.destination.parent.mkdir()
        self.destination.write_bytes(PREVIOUS_DATABASE)
        result = self.fetch("corrupt")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.destination.read_bytes(), PREVIOUS_DATABASE)
        self.assert_attempts(3)

    def test_requires_a_download_token(self):
        result = self.fetch("success", token=None)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("IPINFO_DOWNLOAD_TOKEN", result.stderr)
        self.assert_attempts(0)


if __name__ == "__main__":
    unittest.main()
