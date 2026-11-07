#!/usr/bin/env python3
"""Destructive native integration check, only on an ephemeral Linux Actions runner."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
SOCKET = "/run/driftnet/network.sock"
APP = Path("/usr/lib/driftnet")

def run(*args: str, timeout: int = 120) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
    if result.returncode != 0:
        raise RuntimeError(f"{args[0]} exited {result.returncode}: {(result.stdout + result.stderr)[-5000:]}")
    return result

def wait(check, label: str, seconds: int = 30):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            result = check()
            if result: return result
        except (OSError, ValueError, RuntimeError):
            pass
        time.sleep(0.2)
    raise RuntimeError(f"Timed out: {label}")

def connect() -> tuple[socket.socket, object]:
    channel = socket.socket(socket.AF_UNIX)
    channel.settimeout(30)
    channel.connect(SOCKET)
    return channel, channel.makefile("rwb", buffering=0)

def send(file, request: dict) -> dict:
    file.write(json.dumps(request).encode() + b"\n")
    line = file.readline(8192)
    if not line: raise RuntimeError("Networking service closed before replying")
    return json.loads(line)

def stats() -> dict:
    channel, file = connect()
    try: return send(file, {"action": "stats"})
    finally: file.close(); channel.close()

def curl(path: Path):
    result = run(str(path), "--ipv4", "--noproxy", "*", "--silent", "--show-error", "--fail", "--connect-timeout", "15", "--max-time", "35", "--output", "/dev/null", "--write-out", "%{http_code}", "https://example.com/", timeout=45)
    if result.stdout != "200": raise RuntimeError(f"HTTPS probe returned {result.stdout}")

def tun_exists() -> bool:
    return Path("/sys/class/net/driftnet0").exists()

def test(args):
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.name != "posix" or not Path("/proc/sys/kernel/ostype").exists() or os.geteuid() == 0:
        raise SystemExit("Run only as the ordinary user on an ephemeral Linux GitHub Actions runner.")
    run("sudo", "-n", "true")
    installed = subprocess.run(["dpkg-query", "-W", "-f=${Status}", "driftnet"], capture_output=True, text=True)
    if (installed.returncode == 0 and installed.stdout == "install ok installed") or Path(SOCKET).exists() or APP.exists():
        raise SystemExit("Refusing to modify an existing Driftnet installation.")
    installer = args.installer.resolve(strict=True)
    report = {"os": run("uname", "-a").stdout.strip(), "installer": installer.name,
              "install": False, "serviceMode": None, "engineCapability": False,
              "repairPreservesRules": False, "selectedRoute": False, "unselectedRoute": False,
              "leaseCleanup": False, "crashLeaseCleanup": False, "uninstall": False,
              "websiteAndDiscordVoiceTested": False, "error": None}
    engine = None; fallback = None; lease = None; file = None; package_installed = False
    config = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "app.drift.desktop/rules.json"
    original = config.read_bytes() if config.exists() else None
    fixture = b'{"schema_version":1,"rules":[]}'
    has_systemd = Path("/run/systemd/system").is_dir()
    try:
        run("sudo", "-n", "apt-get", "install", "-y", str(installer), timeout=300)
        package_installed = True
        for name in ["drift-desktop", "drift-agent", "drift-engine", "drift-router"]:
            if not (APP / name).is_file(): raise RuntimeError(f"Installed binary missing: {name}")
        if not Path("/usr/bin/driftnet").is_file(): raise RuntimeError("App launcher missing")
        report["install"] = True
        caps = run("getcap", str(APP / "drift-engine")).stdout
        if "cap_net_raw=ep" not in caps: raise RuntimeError("Installed engine lacks its interface-binding capability")
        report["engineCapability"] = True
        config.parent.mkdir(parents=True, exist_ok=True)
        config.write_bytes(fixture)
        run("sudo", "-n", "dpkg", "--install", str(installer), timeout=180)
        if config.read_bytes() != fixture: raise RuntimeError("Repair installation changed user rules")
        report["repairPreservesRules"] = True
        if has_systemd:
            run("systemctl", "is-active", "--quiet", "driftnet-network.service")
            report["serviceMode"] = "systemd"
        else:
            # The fallback proves the packaged helper/TUN, not systemd installation.
            run("sudo", "-n", "install", "-d", "-m", "0755", "/run/driftnet")
            fallback = subprocess.Popen(["sudo", "-n", "sh", "-c", "echo $$ > /run/driftnet/ci-helper.pid; exec /usr/lib/driftnet/drift-router --serve"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            report["serviceMode"] = "direct-helper-fallback-systemd-unverified"
        wait(lambda: Path(SOCKET).exists(), "installed service socket")
        interface = int(run(str(APP / "drift-router"), "--interface-index").stdout.strip())
        with tempfile.TemporaryDirectory(prefix="driftnet-native-test-") as temporary:
            selected = Path(shutil.which("curl") or "/usr/bin/curl").resolve()
            direct = Path(temporary) / "direct-curl"
            shutil.copy2(selected, direct)
            reservation = socket.socket(); reservation.bind(("127.0.0.1", 0)); port = reservation.getsockname()[1]; reservation.close()
            environment = dict(os.environ, DRIFT_OUTBOUND_IFINDEX=str(interface))
            environment.pop("SS_LOCAL_PORT", None); environment.pop("SS_PLUGIN_OPTIONS", None)
            with open(Path(temporary) / "engine.log", "wb") as log:
                engine = subprocess.Popen([str(APP / "drift-engine"), "--ip", "127.0.0.1", "--port", str(port), "--max-conn", "256", "--split", "2", "--tlsrec", "1+s"], stdin=subprocess.PIPE, stdout=log, stderr=log, env=environment)
                def engine_ready():
                    with socket.create_connection(("127.0.0.1", port), timeout=1) as client:
                        client.sendall(b"\x05\x01\x00"); return client.recv(2) == b"\x05\x00"
                wait(engine_ready, "packaged SOCKS engine", 15)
                request = {"url": "", "engine_port": port, "interface_index": interface, "apps": [{"path": str(selected), "bundle": False}]}
                lease, file = connect()
                response = send(file, request)
                if not response.get("ok") or not response.get("router") or response.get("protocol") != 2:
                    raise RuntimeError(f"TUN did not start: {response}")
                wait(tun_exists, "TUN interface")
                before = stats(); curl(selected); after_selected = stats()
                if after_selected["selected"] <= before["selected"]: raise RuntimeError("Selected executable did not use the selected route")
                report["selectedRoute"] = True
                curl(direct); after_direct = stats()
                if after_direct["direct"] <= after_selected["direct"] or after_direct["selected"] != after_selected["selected"]:
                    raise RuntimeError("Unselected executable was not attributed to direct routing")
                report["unselectedRoute"] = True; report["stats"] = after_direct
                file.write(b"stop\n")
                if not json.loads(file.readline(8192)).get("ok"): raise RuntimeError("TUN teardown was not acknowledged")
                file.close(); lease.close(); file = None; lease = None
                wait(lambda: not tun_exists(), "TUN removed after stop"); curl(selected)
                report["leaseCleanup"] = True
                lease, file = connect()
                if not send(file, request).get("router"): raise RuntimeError("Second TUN session could not start")
                file.close(); lease.close(); file = None; lease = None
                wait(lambda: not tun_exists(), "TUN removed after control EOF"); curl(selected)
                report["crashLeaseCleanup"] = True
                engine.stdin.close(); engine.wait(timeout=10); engine = None
        if fallback is not None:
            pid = Path("/run/driftnet/ci-helper.pid").read_text().strip()
            run("sudo", "-n", "kill", "-TERM", pid); fallback.wait(timeout=15); fallback = None
            run("sudo", "-n", "rm", "-rf", "/run/driftnet")
        run("sudo", "-n", "dpkg", "--remove", "driftnet", timeout=120)
        package_installed = False
        if APP.exists() or Path("/usr/bin/driftnet").exists() or Path(SOCKET).exists(): raise RuntimeError("Uninstall left installed binaries or networking socket")
        if config.read_bytes() != fixture: raise RuntimeError("Uninstall removed user rules")
        report["uninstall"] = True
    except BaseException as error:
        report["error"] = str(error)
        raise
    finally:
        if file is not None: file.close()
        if lease is not None: lease.close()
        if engine is not None:
            engine.stdin.close()
            try: engine.wait(timeout=10)
            except subprocess.TimeoutExpired: engine.kill(); engine.wait()
        if fallback is not None:
            try: run("sudo", "-n", "kill", "-TERM", Path("/run/driftnet/ci-helper.pid").read_text().strip()); fallback.wait(timeout=15)
            except (OSError, subprocess.SubprocessError): pass
            subprocess.run(["sudo", "-n", "rm", "-rf", "/run/driftnet"], check=False)
        if package_installed: subprocess.run(["sudo", "-n", "dpkg", "--remove", "driftnet"], check=False)
        if original is None: config.unlink(missing_ok=True)
        else: config.write_bytes(original)
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--installer", type=Path, required=True)
    parser.add_argument("--report", type=Path, default=ROOT / "target/linux-installer-test.json")
    test(parser.parse_args())
