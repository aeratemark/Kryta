#!/usr/bin/env python3
# run_server.py - a window that builds and runs this machine's Kryta servers.
#
# What it starts is data, not code: the services are read from tools/supervisor/stack.json, the
# same list `cargo run -p kryta-supervisor` starts, so a port or an argument changes there and
# nowhere else. The control panel (kryta-management-service) is offered beside them if stack.json lacks it.
#
# For each service it can build (`cargo build -p <program>`), start the built binary from the
# repository root with the arguments stack.json gives it, stop it, and show what it prints. A
# service is "ready" when it prints its READY line, the same line the supervisor waits for.
#
# It changes nothing in the tree on its own. The one write it can make is "Create from .dist",
# which copies a missing config file from its .dist beside it, and only when that button is pressed.
#
# Needs Python 3.8+ with tkinter (part of the standard Windows installer). No other modules.
#
#   python run_server.py            # from anywhere; it finds the repository beside itself
#   python run_server.py --root D:\path\to\Kryta-main

import argparse
import glob
import json
import os
import queue
import re
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
import tkinter as tk
from tkinter import messagebox, ttk
from tkinter.scrolledtext import ScrolledText

STACK_FILE = os.path.join("tools", "supervisor", "stack.json")
READY_MARK = "READY:"
# What the worldserver prints when a schema file changed after the database applied it
# (database/src/Updater/Apply.rs, ApplyError::Modified).
SCHEMA_MODIFIED_MARK = "was modified after it was applied"
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
MAX_LOG_LINES = 5000
STOP_GRACE_SECONDS = 5.0
READY_TIMEOUT_SECONDS = 60.0
# The reference's exit-code contract (Main.cpp:434-439, read by tools/supervisor): a program that exits
# 2 asked to be restarted. The launcher honours it as the supervisor does, up to this many times.
RESTART_EXIT_CODE = 2
MAX_RESTARTS = 5
# The world's operator client (worldserver's second binary) and the credential its control port needs.
CLI_PROGRAM = "kryta-cli"
CLI_PACKAGE = "worldserver"
CREDENTIAL_FILE = "worldserver/control.credential"

# Offered beside the stack's services when stack.json does not list them (older trees did not).
# The control panel's executable is kryta-management-service; kryta-manage is the same program
# under its old name, and offering both would start two panels fighting over port 6602.
EXTRA_SERVICES = [
    {
        "name": "manage",
        "program": "kryta-management-service",
        "args": ["-c", "tools/manage/manage.json"],
        "port": 6602,
    },
]

FALLBACK_SERVICES = [
    {"name": "auth", "program": "authserver", "args": ["-c", "authserver/authserver.json"], "port": 6112},
    {"name": "world", "program": "worldserver", "args": ["-c", "worldserver/worldserver.json"], "port": 6113},
]

IS_WINDOWS = os.name == "nt"


def find_cargo():
    """cargo on PATH, else the usual rustup location."""
    found = shutil.which("cargo")
    if found:
        return found
    home = os.path.expanduser("~")
    candidate = os.path.join(home, ".cargo", "bin", "cargo.exe" if IS_WINDOWS else "cargo")
    return candidate if os.path.exists(candidate) else "cargo"


def load_services(root):
    """The services in stack.json, then the extras. Answers (services, note)."""
    path = os.path.join(root, STACK_FILE)
    try:
        with open(path, "r", encoding="utf-8") as handle:
            stack = json.load(handle)
        services = [dict(s, in_stack=True) for s in stack.get("services", [])]
        note = "services from " + STACK_FILE
    except (OSError, ValueError) as error:
        services = [dict(s, in_stack=True) for s in FALLBACK_SERVICES]
        note = "cannot read %s (%s); using the built-in list" % (STACK_FILE, error)
    known = {s["program"] for s in services}
    for extra in EXTRA_SERVICES:
        if extra["program"] not in known:
            services.append(dict(extra, in_stack=False))
    return services, note


def config_of(service):
    """The path after -c / --config in a service's arguments, if any."""
    args = service.get("args", [])
    for index, arg in enumerate(args):
        if arg in ("-c", "--config") and index + 1 < len(args):
            return args[index + 1]
        if arg.startswith("--config="):
            return arg.split("=", 1)[1]
    return None


def find_psql():
    """psql on PATH, else the newest one a Windows PostgreSQL installer put in Program Files."""
    found = shutil.which("psql")
    if found:
        return found
    patterns = []
    for base in (os.environ.get("ProgramFiles"), os.environ.get("ProgramFiles(x86)")):
        if base:
            patterns.append(os.path.join(base, "PostgreSQL", "*", "bin", "psql.exe"))
    patterns += ["/usr/lib/postgresql/*/bin/psql", "/Library/PostgreSQL/*/bin/psql"]
    candidates = []
    for pattern in patterns:
        candidates += glob.glob(pattern)

    def version(path):
        part = os.path.basename(os.path.dirname(os.path.dirname(path)))
        try:
            return float(part)
        except ValueError:
            return 0.0

    return max(candidates, key=version) if candidates else None


def read_database_infos(root, config):
    """Every *DatabaseInfo string in a program's JSON config: [(key, info string)]."""
    try:
        with open(os.path.join(root, config), "r", encoding="utf-8") as handle:
            document = json.load(handle)
    except (OSError, ValueError):
        return []
    found = []
    for key, value in document.items():
        if not key.endswith("DatabaseInfo"):
            continue
        if isinstance(value, dict):
            value = value.get("value")
        if isinstance(value, str) and value.strip():
            found.append((key, value.strip()))
    return found


# What psql's answer means, and what to do about it.
DB_HINTS = [
    ("password authentication failed",
     "either the role does not exist yet or its password is not the one in the config (PostgreSQL "
     "says the same for both; the .dist ships 'change-me') - press Database help for the SQL"),
    ("role", "the role does not exist; create it (see the SQL the launcher's Database help prints)"),
    ("database", "the database does not exist; create it (empty is enough, the programs make their tables)"),
    ("no pg_hba.conf entry", "pg_hba.conf refuses this host/user/database; allow it for 127.0.0.1"),
    ("connection refused", "nothing listens there; start the PostgreSQL service"),
    ("timeout", "nothing answered; check the host, port and firewall"),
]


def hint_for(message):
    lower = message.lower()
    if "password authentication failed" in lower:
        return DB_HINTS[0][1]
    if "role" in lower and "does not exist" in lower:
        return DB_HINTS[1][1]
    if "database" in lower and "does not exist" in lower:
        return DB_HINTS[2][1]
    for needle, hint in DB_HINTS[3:]:
        if needle in lower:
            return hint
    return None


def setting_keys(document, prefix=""):
    """Every setting name in a config document, nested groups joined with dots."""
    keys = set()
    for key, value in document.items():
        if key in ("note", "information"):
            continue
        if isinstance(value, dict) and "value" not in value:
            keys |= setting_keys(value, prefix + key + ".")
        else:
            keys.add(prefix + key)
    return keys


def missing_settings(root, config):
    """Settings the .dist has and the live config does not: the ones a repo update added."""
    try:
        with open(os.path.join(root, config), "r", encoding="utf-8") as handle:
            live = json.load(handle)
        with open(os.path.join(root, config + ".dist"), "r", encoding="utf-8") as handle:
            dist = json.load(handle)
    except (OSError, ValueError):
        return []
    return sorted(setting_keys(dist) - setting_keys(live))


def read_setting(root, config, dotted):
    """One setting's value from a config file, or None."""
    try:
        with open(os.path.join(root, config), "r", encoding="utf-8") as handle:
            node = json.load(handle)
    except (OSError, ValueError):
        return None
    for part in dotted.split("."):
        if not isinstance(node, dict) or part not in node:
            return None
        node = node[part]
    return node.get("value") if isinstance(node, dict) else node


class Service:
    """One program: its settings, its process and its state."""

    def __init__(self, spec):
        self.name = spec["name"]
        self.program = spec["program"]
        self.args = list(spec.get("args", []))
        self.port = spec.get("port")
        self.config = config_of(spec)
        self.in_stack = spec.get("in_stack", True)
        self.restarts = 0
        # Arguments for the next start only (e.g. --recheck-schema), cleared once it has started.
        self.once_args = []
        # Set when the program's output says a schema file changed after it was applied.
        self.schema_modified = False
        self.process = None
        self.state = "stopped"
        self.stopping = False


class Launcher:
    def __init__(self, master, root):
        self.master = master
        self.root = root
        self.cargo = find_cargo()
        self.events = queue.Queue()
        self.build_lock = threading.Lock()
        self.cargo_busy = False
        self.cargo_process = None

        specs, note = load_services(root)
        self.services = [Service(spec) for spec in specs]
        self.by_name = {s.name: s for s in self.services}

        self.profile = tk.StringVar(value="debug")
        self.build_first = tk.BooleanVar(value=True)
        self.autoscroll = tk.BooleanVar(value=True)

        master.title("Kryta - run_server")
        master.geometry("1100x720")
        master.minsize(800, 500)
        master.protocol("WM_DELETE_WINDOW", self.on_close)

        self._build_toolbar()
        self._build_service_table()
        self._build_world_commands()
        self._build_logs()
        self.status = ttk.Label(master, anchor="w", relief="sunken")
        self.status.pack(fill="x", side="bottom")

        self.log("launcher", "repository: " + root)
        self.log("launcher", "cargo: " + self.cargo)
        self.log("launcher", note)
        self.report_config_drift()
        self.refresh_all()
        self.master.after(100, self.pump)

    # ---- layout -------------------------------------------------------------------------------

    def _build_toolbar(self):
        bar = ttk.Frame(self.master, padding=(6, 6, 6, 0))
        bar.pack(fill="x")

        ttk.Label(bar, text="Profile:").pack(side="left")
        for value in ("debug", "release"):
            ttk.Radiobutton(bar, text=value, value=value, variable=self.profile,
                            command=self.refresh_all).pack(side="left")
        ttk.Checkbutton(bar, text="Build before start", variable=self.build_first).pack(side="left", padx=(10, 0))

        ttk.Separator(bar, orient="vertical").pack(side="left", fill="y", padx=8)
        self.cargo_buttons = []
        for label, args in (
            ("Build all", ["build", "--workspace"]),
            ("Check", ["check", "--workspace"]),
        ):
            button = ttk.Button(bar, text=label, command=lambda a=args, l=label: self.run_cargo(l, a))
            button.pack(side="left", padx=2)
            self.cargo_buttons.append(button)
        self.cancel_button = ttk.Button(bar, text="Cancel cargo", command=self.cancel_cargo, state="disabled")
        self.cancel_button.pack(side="left", padx=2)

        ttk.Separator(bar, orient="vertical").pack(side="left", fill="y", padx=8)
        ttk.Button(bar, text="Start servers", command=self.start_stack).pack(side="left", padx=2)
        ttk.Button(bar, text="Stop servers", command=self.stop_all).pack(side="left", padx=2)
        ttk.Separator(bar, orient="vertical").pack(side="left", fill="y", padx=8)
        ttk.Button(bar, text="Check database", command=self.check_databases).pack(side="left", padx=2)
        ttk.Button(bar, text="Database help", command=self.database_help).pack(side="left", padx=2)
        ttk.Button(bar, text="Recheck schema", command=self.recheck_schema).pack(side="left", padx=2)

        ttk.Button(bar, text="Open folder", command=self.open_folder).pack(side="right", padx=2)

    def _build_service_table(self):
        frame = ttk.LabelFrame(self.master, text="Services", padding=6)
        frame.pack(fill="x", padx=6, pady=6)
        headers = ("Service", "Program", "Port", "State", "Binary", "Config", "")
        for column, text in enumerate(headers):
            ttk.Label(frame, text=text, font=("TkDefaultFont", 9, "bold")).grid(
                row=0, column=column, sticky="w", padx=4)
        self.rows = {}
        for index, service in enumerate(self.services, start=1):
            widgets = {}
            ttk.Label(frame, text=service.name).grid(row=index, column=0, sticky="w", padx=4)
            ttk.Label(frame, text=service.program).grid(row=index, column=1, sticky="w", padx=4)
            ttk.Label(frame, text=str(service.port or "")).grid(row=index, column=2, sticky="w", padx=4)
            widgets["state"] = tk.Label(frame, text="stopped", width=12, anchor="w")
            widgets["state"].grid(row=index, column=3, sticky="w", padx=4)
            widgets["binary"] = ttk.Label(frame, text="")
            widgets["binary"].grid(row=index, column=4, sticky="w", padx=4)
            config_cell = ttk.Frame(frame)
            config_cell.grid(row=index, column=5, sticky="w", padx=4)
            widgets["config"] = ttk.Label(config_cell, text="")
            widgets["config"].pack(side="left")
            widgets["dist"] = ttk.Button(config_cell, text="Create from .dist",
                                         command=lambda s=service: self.create_config(s))
            buttons = ttk.Frame(frame)
            buttons.grid(row=index, column=6, sticky="e", padx=4)
            for label, action in (("Build", self.build_one), ("Start", self.start_one),
                                  ("Stop", self.stop_one), ("Restart", self.restart_one)):
                ttk.Button(buttons, text=label, width=8,
                           command=lambda s=service, a=action: a(s)).pack(side="left", padx=1)
            self.rows[service.name] = widgets
        frame.columnconfigure(5, weight=1)

    def _build_world_commands(self):
        frame = ttk.LabelFrame(self.master, text="World commands (kryta-cli)", padding=6)
        frame.pack(fill="x", padx=6, pady=(0, 6))
        ttk.Label(frame, text="Command:").pack(side="left")
        self.cli_text = tk.StringVar(value="list")
        entry = ttk.Entry(frame, textvariable=self.cli_text, width=50)
        entry.pack(side="left", padx=4)
        entry.bind("<Return>", lambda _event: self.run_cli())
        ttk.Button(frame, text="Run", command=self.run_cli).pack(side="left", padx=2)
        ttk.Label(frame, text="e.g. list, or <operation> name=value ...").pack(side="left", padx=6)
        ttk.Button(frame, text="Create control credential",
                   command=self.create_credential).pack(side="right", padx=2)

    def _build_logs(self):
        frame = ttk.Frame(self.master, padding=(6, 0, 6, 6))
        frame.pack(fill="both", expand=True)
        controls = ttk.Frame(frame)
        controls.pack(fill="x")
        ttk.Checkbutton(controls, text="Auto-scroll", variable=self.autoscroll).pack(side="left")
        ttk.Button(controls, text="Clear tab", command=self.clear_tab).pack(side="left", padx=4)
        ttk.Label(controls, text="Filter:").pack(side="left", padx=(12, 2))
        self.filter_text = tk.StringVar()
        ttk.Entry(controls, textvariable=self.filter_text, width=30).pack(side="left")
        ttk.Label(controls, text="(new lines only)").pack(side="left", padx=4)

        self.tabs = ttk.Notebook(frame)
        self.tabs.pack(fill="both", expand=True, pady=(4, 0))
        self.views = {}
        for name in ["all", "cargo"] + [s.name for s in self.services]:
            text = ScrolledText(self.tabs, wrap="none", font=("Consolas" if IS_WINDOWS else "Monospace", 9),
                                background="#1e1e1e", foreground="#d4d4d4", insertbackground="#d4d4d4")
            text.tag_configure("error", foreground="#f48771")
            text.tag_configure("warn", foreground="#dcdcaa")
            text.tag_configure("ready", foreground="#6a9955")
            text.tag_configure("meta", foreground="#569cd6")
            text.configure(state="disabled")
            self.tabs.add(text, text=name)
            self.views[name] = text

    # ---- logging ------------------------------------------------------------------------------

    def log(self, source, line):
        """Queue one line from any thread."""
        self.events.put(("line", source, line))

    def _append(self, source, line):
        line = ANSI.sub("", line.rstrip("\r\n"))
        wanted = self.filter_text.get().strip().lower()
        if wanted and wanted not in line.lower() and source != "launcher":
            return
        lower = line.lower()
        if source == "launcher":
            tag = "meta"
        elif READY_MARK in line:
            tag = "ready"
        elif "error" in lower or "panicked" in lower or "fatal" in lower:
            tag = "error"
        elif "warn" in lower:
            tag = "warn"
        else:
            tag = None
        stamp = time.strftime("%H:%M:%S")
        targets = ["all"]
        if source in self.views and source != "all":
            targets.append(source)
        elif source == "launcher":
            pass
        for target in targets:
            text = self.views[target]
            prefix = "%s %-8s| " % (stamp, source) if target == "all" else "%s | " % stamp
            text.configure(state="normal")
            text.insert("end", prefix + line + "\n", tag)
            excess = int(text.index("end-1c").split(".")[0]) - MAX_LOG_LINES
            if excess > 0:
                text.delete("1.0", "%d.0" % (excess + 1))
            text.configure(state="disabled")
            if self.autoscroll.get():
                text.see("end")

    def clear_tab(self):
        current = self.tabs.select()
        for text in self.views.values():
            if str(text) == current:
                text.configure(state="normal")
                text.delete("1.0", "end")
                text.configure(state="disabled")

    # ---- the event pump (everything that touches widgets runs here) ---------------------------

    def pump(self):
        try:
            for _ in range(500):
                kind, *rest = self.events.get_nowait()
                if kind == "line":
                    self._append(*rest)
                elif kind == "state":
                    name, state = rest
                    self.by_name[name].state = state
                    self.refresh(self.by_name[name])
                elif kind == "cargo":
                    self.set_cargo_busy(rest[0])
                elif kind == "call":
                    rest[0]()
        except queue.Empty:
            pass
        self.master.after(50, self.pump)

    def set_state(self, service, state):
        self.events.put(("state", service.name, state))

    # ---- paths --------------------------------------------------------------------------------

    def exe_of(self, service):
        name = service.program + (".exe" if IS_WINDOWS else "")
        return os.path.join(self.root, "target", self.profile.get(), name)

    def refresh_all(self):
        for service in self.services:
            self.refresh(service)

    def refresh(self, service):
        row = self.rows[service.name]
        colours = {"stopped": "#808080", "building": "#c08000", "starting": "#c08000",
                   "ready": "#008000", "running": "#2060c0", "stopping": "#c08000",
                   "failed": "#c00000"}
        colour = colours.get(service.state.split(" ")[0], "#c00000")
        row["state"].configure(text=service.state, fg=colour)
        exe = self.exe_of(service)
        if os.path.exists(exe):
            built = time.strftime("%Y-%m-%d %H:%M", time.localtime(os.path.getmtime(exe)))
            row["binary"].configure(text="built " + built)
        else:
            row["binary"].configure(text="not built")
        if service.config:
            present = os.path.exists(os.path.join(self.root, service.config))
            note = "" if present else "  (missing)"
            if present:
                new = missing_settings(self.root, service.config)
                if new:
                    note = "  (%d new setting(s) in .dist)" % len(new)
            row["config"].configure(text=service.config + note)
            has_dist = os.path.exists(os.path.join(self.root, service.config + ".dist"))
            if not present and has_dist:
                row["dist"].pack(side="left", padx=4)
            else:
                row["dist"].pack_forget()
        else:
            row["config"].configure(text="(none)")

    def create_config(self, service):
        target = os.path.join(self.root, service.config)
        source = target + ".dist"
        if os.path.exists(target):
            self.refresh(service)
            return
        if not messagebox.askyesno("Create config",
                                   "Copy %s.dist to %s?\n\nEdit it afterwards with this machine's "
                                   "database settings." % (service.config, service.config)):
            return
        shutil.copyfile(source, target)
        self.log("launcher", "created %s from its .dist" % service.config)
        self.refresh(service)

    # ---- cargo --------------------------------------------------------------------------------

    def set_cargo_busy(self, busy):
        self.cargo_busy = busy
        for button in self.cargo_buttons:
            button.configure(state="disabled" if busy else "normal")
        self.cancel_button.configure(state="normal" if busy else "disabled")
        self.status.configure(text="cargo is running..." if busy else "")
        if not busy:
            self.refresh_all()

    def cargo_args(self, args):
        if self.profile.get() == "release" and args[0] in ("build", "check", "clippy", "test"):
            return args + ["--release"]
        return args

    def _run_cargo_blocking(self, label, args, log_to="cargo"):
        """Run one cargo command, one at a time; answers True when it succeeded."""
        with self.build_lock:
            self.events.put(("cargo", True))
            command = [self.cargo] + self.cargo_args(args)
            self.log("launcher", "%s: %s" % (label, " ".join(command[1:])))
            started = time.time()
            try:
                process = subprocess.Popen(command, cwd=self.root, stdout=subprocess.PIPE,
                                           stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                                           **self.popen_flags())
            except OSError as error:
                self.log("launcher", "cannot run cargo: %s" % error)
                self.events.put(("cargo", False))
                return False
            self.cargo_process = process
            for raw in iter(process.stdout.readline, b""):
                self.log(log_to, raw.decode("utf-8", "replace"))
            code = process.wait()
            self.cargo_process = None
            self.log("launcher", "%s %s in %.1f s (exit %d)" % (
                label, "finished" if code == 0 else "FAILED", time.time() - started, code))
            self.events.put(("cargo", False))
            return code == 0

    def run_cargo(self, label, args):
        threading.Thread(target=self._run_cargo_blocking, args=(label, args), daemon=True).start()

    def cancel_cargo(self):
        process = self.cargo_process
        if process and process.poll() is None:
            self.log("launcher", "cancelling cargo")
            process.kill()

    def build_one(self, service):
        def work():
            self.set_state(service, "building")
            ok = self._run_cargo_blocking("build " + service.program, ["build", "-p", service.program])
            self.set_state(service, "stopped" if ok else "failed (build)")
        threading.Thread(target=work, daemon=True).start()

    # ---- processes ----------------------------------------------------------------------------

    @staticmethod
    def popen_flags():
        if IS_WINDOWS:
            # Own process group, so CTRL_BREAK can reach the program without reaching this window.
            return {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
        return {"start_new_session": True}

    def is_running(self, service):
        return service.process is not None and service.process.poll() is None

    def start_one(self, service, then=None, build=None, manual=True):
        if build is None:
            build = self.build_first.get()
        if manual:
            service.restarts = 0
        if self.is_running(service):
            self.log("launcher", "%s is already running" % service.name)
            return

        def work():
            if build:
                self.set_state(service, "building")
                if not self._run_cargo_blocking("build " + service.program,
                                                ["build", "-p", service.program]):
                    self.set_state(service, "failed (build)")
                    return
            exe = self.exe_of(service)
            if not os.path.exists(exe):
                self.log("launcher", "%s is not built: %s" % (service.name, exe))
                self.set_state(service, "failed (no binary)")
                return
            if service.config and not os.path.exists(os.path.join(self.root, service.config)):
                self.log("launcher", "warning: %s has no %s; it will say why it cannot start"
                         % (service.name, service.config))
            self.set_state(service, "starting")
            args = service.args + service.once_args
            service.once_args = []
            service.schema_modified = False
            self.log("launcher", "starting %s: %s %s" % (service.name, exe, " ".join(args)))
            try:
                process = subprocess.Popen([exe] + args, cwd=self.root,
                                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                           stdin=subprocess.DEVNULL, **self.popen_flags())
            except OSError as error:
                self.log("launcher", "cannot start %s: %s" % (service.name, error))
                self.set_state(service, "failed (start)")
                return
            service.process = process
            service.stopping = False
            if then:
                self.events.put(("call", then))
            for raw in iter(process.stdout.readline, b""):
                line = raw.decode("utf-8", "replace")
                if READY_MARK in line and not service.stopping:
                    self.set_state(service, "ready")
                if SCHEMA_MODIFIED_MARK in line:
                    service.schema_modified = True
                self.log(service.name, line)
            code = process.wait()
            service.process = None
            if service.stopping:
                self.set_state(service, "stopped")
                self.log("launcher", "%s stopped (exit %d)" % (service.name, code))
            elif code == RESTART_EXIT_CODE and service.restarts < MAX_RESTARTS:
                service.restarts += 1
                self.log("launcher", "%s exited with code %d, which asks for a restart; starting it "
                         "again (%d of %d)" % (service.name, code, service.restarts, MAX_RESTARTS))
                self.events.put(("call", lambda: self.start_one(service, build=False, manual=False)))
            else:
                self.set_state(service, "stopped" if code == 0 else "exited %d" % code)
                self.log("launcher", "%s exited on its own with code %d" % (service.name, code))
                if service.schema_modified and service.program == "worldserver":
                    self.events.put(("call", lambda: self.offer_recheck(service)))

        threading.Thread(target=work, daemon=True).start()

    def stop_one(self, service, then=None):
        process = service.process
        if process is None or process.poll() is not None:
            if then:
                then()
            return
        service.stopping = True
        self.set_state(service, "stopping")
        self.log("launcher", "stopping %s" % service.name)

        def work():
            try:
                if IS_WINDOWS:
                    process.send_signal(signal.CTRL_BREAK_EVENT)
                else:
                    os.killpg(process.pid, signal.SIGINT)
            except (OSError, ValueError):
                pass
            try:
                process.wait(timeout=STOP_GRACE_SECONDS)
            except subprocess.TimeoutExpired:
                self.log("launcher", "%s did not stop within %.0f s; killing it"
                         % (service.name, STOP_GRACE_SECONDS))
                process.kill()
                process.wait()
            if then:
                self.events.put(("call", then))

        threading.Thread(target=work, daemon=True).start()

    def restart_one(self, service):
        self.stop_one(service, then=lambda: self.start_one(service))

    def offer_recheck(self, service):
        """The world stopped because a schema file it applied earlier has changed (usually after
        pulling an update). Offer to start it once with --recheck-schema, which re-records the
        files' fingerprints and deletes nothing; the next normal start checks them again."""
        if not messagebox.askyesno(
                "Schema files changed",
                "%s stopped because one or more SQL files in data/sql/base were changed after "
                "the database applied them (this is normal after updating from GitHub).\n\n"
                "Start it once with --recheck-schema? That accepts the files as they are now and "
                "deletes no data. New migration files still apply as usual." % service.name):
            self.log("launcher", "recheck declined; %s stays stopped" % service.name)
            return
        self.recheck_schema(service)

    def recheck_schema(self, service=None):
        """Start the world once with --recheck-schema (stopping it first if it runs)."""
        if service is None:
            service = next((s for s in self.services if s.program == "worldserver"), None)
            if service is None:
                self.log("launcher", "no worldserver service to recheck")
                return
        self.log("launcher", "starting %s once with --recheck-schema" % service.name)

        def go():
            service.once_args = ["--recheck-schema"]
            self.start_one(service)

        self.stop_one(service, then=lambda: self.events.put(("call", go)))

    def start_stack(self):
        """Start what stack.json lists, in its order, each after the last one's READY."""
        stack = [s for s in self.services if s.in_stack]

        def work():
            if not self._check_databases_blocking():
                self.log("launcher", "the database check failed; starting anyway so the servers "
                         "say it in their own words")
            if self.build_first.get():
                programs = []
                for service in stack:
                    programs += ["-p", service.program]
                for service in stack:
                    self.set_state(service, "building")
                ok = self._run_cargo_blocking("build servers", ["build"] + programs)
                for service in stack:
                    self.set_state(service, "stopped" if ok else "failed (build)")
                if not ok:
                    return
            # One at a time, each after the last one's READY: two programs that update the same
            # fresh database at once collide on its first table.
            for service in stack:
                service.state = "queued"
                self.events.put(("call", lambda s=service: self.start_one(s, build=False)))
                deadline = time.time() + READY_TIMEOUT_SECONDS
                while time.time() < deadline:
                    time.sleep(0.2)
                    if service.state == "ready":
                        break
                    if service.state.startswith(("exited", "failed", "stopped")) and service.state != "queued":
                        break
                if service.state != "ready":
                    self.log("launcher", "%s did not reach READY (%s); not starting the rest"
                             % (service.name, service.state))
                    return
            self.log("launcher", "the stack is up: " + ", ".join(
                "%s on %s" % (s.name, s.port) for s in stack))

        threading.Thread(target=work, daemon=True).start()

    def stop_all(self):
        for service in self.services:
            self.stop_one(service)

    # ---- the database -------------------------------------------------------------------------

    def database_targets(self):
        """Every distinct database the configured programs name: [(info, [who names it])]."""
        targets = {}
        for service in self.services:
            if not service.config:
                continue
            for key, info in read_database_infos(self.root, service.config):
                targets.setdefault(info, []).append("%s %s" % (service.name, key))
        return list(targets.items())

    def _check_databases_blocking(self):
        """Log, for each database, whether it answers and psql's own words when it does not."""
        targets = self.database_targets()
        if not targets:
            self.log("launcher", "database check: no *DatabaseInfo found (are the configs created?)")
            return False
        psql = find_psql()
        if psql is None:
            self.log("launcher", "database check: psql not found on PATH or in Program Files; "
                     "only the port can be checked")
        all_ok = True
        for info, users in targets:
            fields = info.split(";")
            if len(fields) != 5:
                self.log("launcher", "database check: '%s' is not host;port;user;password;database (%s)"
                         % (fields[0], ", ".join(users)))
                all_ok = False
                continue
            host, port, user, password, database = fields
            where = "%s@%s:%s/%s" % (user, host, port, database)
            try:
                with socket.create_connection((host, int(port)), timeout=3):
                    pass
            except (OSError, ValueError) as error:
                self.log("launcher", "database %s: port not reachable (%s) - %s"
                         % (where, error, hint_for(str(error)) or "is the PostgreSQL service running?"))
                all_ok = False
                continue
            if psql is None:
                self.log("launcher", "database %s: port answers (login not checked without psql)" % where)
                continue
            env = dict(os.environ, PGPASSWORD=password, PGCONNECT_TIMEOUT="5")
            try:
                result = subprocess.run([psql, "-h", host, "-p", port, "-U", user, "-d", database,
                                         "-w", "-X", "-A", "-t", "-c", "select 1"],
                                        env=env, capture_output=True, timeout=15,
                                        stdin=subprocess.DEVNULL,
                                        creationflags=subprocess.CREATE_NO_WINDOW if IS_WINDOWS else 0)
            except (OSError, subprocess.TimeoutExpired) as error:
                self.log("launcher", "database %s: psql did not run (%s)" % (where, error))
                all_ok = False
                continue
            if result.returncode == 0:
                self.log("launcher", "database %s: OK" % where)
                continue
            all_ok = False
            message = result.stderr.decode("utf-8", "replace").strip().replace("\n", " ")
            self.log("launcher", "database %s: FAILED - %s" % (where, message))
            hint = hint_for(message)
            if hint:
                self.log("launcher", "  -> " + hint)
        return all_ok

    def check_databases(self):
        threading.Thread(target=self._check_databases_blocking, daemon=True).start()

    def database_help(self):
        """Print the SQL that makes the role and the three databases, with the config's own names."""
        users, databases, password = set(), [], "change-me"
        for info, _ in self.database_targets():
            fields = info.split(";")
            if len(fields) == 5:
                users.add(fields[2])
                password = fields[3]
                if fields[4] not in databases:
                    databases.append(fields[4])
        user = sorted(users)[0] if users else "kryta"
        databases = databases or ["kryta_login", "kryta_world", "kryta_characters"]
        lines = [
            "Database setup, as a PostgreSQL superuser (e.g. psql -U postgres -h 127.0.0.1):",
            "  CREATE ROLE %s LOGIN CREATEDB PASSWORD '<the password in your configs>';" % user,
        ]
        lines += ["  CREATE DATABASE %s OWNER %s;" % (name, user) for name in databases]
        lines += [
            "The programs make their own tables at start-up; empty databases are enough.",
            "If the role exists already: ALTER ROLE %s WITH PASSWORD '<password>';" % user,
        ]
        if password == "change-me":
            lines.append("Note: the configs still hold the .dist password 'change-me'.")
        for line in lines:
            self.log("launcher", line)

    # ---- configs and the world's operator client -------------------------------------------

    def report_config_drift(self):
        for service in self.services:
            if not service.config or not os.path.exists(os.path.join(self.root, service.config)):
                continue
            new = missing_settings(self.root, service.config)
            if new:
                self.log("launcher", "%s: %s lacks %d setting(s) its .dist now has (the program uses "
                         "their defaults): %s" % (service.name, service.config, len(new), ", ".join(new)))

    def cli_exe(self):
        name = CLI_PROGRAM + (".exe" if IS_WINDOWS else "")
        return os.path.join(self.root, "target", self.profile.get(), name)

    def _run_cli_blocking(self, args):
        exe = self.cli_exe()
        if not os.path.exists(exe) or self.build_first.get():
            if not self._run_cargo_blocking("build " + CLI_PROGRAM,
                                            ["build", "-p", CLI_PACKAGE, "--bin", CLI_PROGRAM]):
                return
        self.log("launcher", "%s %s" % (CLI_PROGRAM, " ".join(args)))
        try:
            result = subprocess.run([exe] + args, cwd=self.root, capture_output=True, timeout=30,
                                    stdin=subprocess.DEVNULL,
                                    creationflags=subprocess.CREATE_NO_WINDOW if IS_WINDOWS else 0)
        except (OSError, subprocess.TimeoutExpired) as error:
            self.log("launcher", "%s did not run: %s" % (CLI_PROGRAM, error))
            return
        output = (result.stdout + result.stderr).decode("utf-8", "replace")
        for line in output.splitlines():
            self.log("world", "cli> " + line)
        if result.returncode != 0:
            self.log("launcher", "%s answered exit %d" % (CLI_PROGRAM, result.returncode))
            if read_setting(self.root, "worldserver/worldserver.json", "Control.Enable") is not True:
                self.log("launcher", "  -> the world's control port is off: set Control.Enable to true "
                         "and Control.CredentialFile to %s in worldserver/worldserver.json, then "
                         "restart world" % CREDENTIAL_FILE)

    def run_cli(self):
        try:
            words = shlex.split(self.cli_text.get(), posix=True)
        except ValueError as error:
            self.log("launcher", "cannot read the command: %s" % error)
            return
        if not words:
            return
        args = [words[0]]
        for word in words[1:]:
            args += ["--arg", word] if "=" in word and not word.startswith("--") else [word]
        threading.Thread(target=self._run_cli_blocking, args=(args,), daemon=True).start()

    def create_credential(self):
        path = os.path.join(self.root, CREDENTIAL_FILE)
        if os.path.exists(path):
            self.log("launcher", "%s exists already; leaving it as it is" % CREDENTIAL_FILE)
            return
        threading.Thread(target=self._run_cli_blocking,
                         args=(["init-credential", CREDENTIAL_FILE],), daemon=True).start()

    # ---- misc ---------------------------------------------------------------------------------

    def open_folder(self):
        if IS_WINDOWS:
            os.startfile(self.root)  # noqa: S606 - opens Explorer on the repository
        elif sys.platform == "darwin":
            subprocess.Popen(["open", self.root])
        else:
            subprocess.Popen(["xdg-open", self.root])

    def on_close(self):
        running = [s.name for s in self.services if self.is_running(s)]
        if running and not messagebox.askyesno(
                "Quit", "Stop %s and quit?" % ", ".join(running)):
            return
        for service in self.services:
            if self.is_running(service):
                service.stopping = True
                try:
                    if IS_WINDOWS:
                        service.process.send_signal(signal.CTRL_BREAK_EVENT)
                    else:
                        os.killpg(service.process.pid, signal.SIGINT)
                except (OSError, ValueError):
                    pass
        deadline = time.time() + STOP_GRACE_SECONDS
        for service in self.services:
            process = service.process
            if process is None:
                continue
            try:
                process.wait(timeout=max(0.1, deadline - time.time()))
            except subprocess.TimeoutExpired:
                process.kill()
        if self.cargo_process and self.cargo_process.poll() is None:
            self.cargo_process.kill()
        self.master.destroy()


def main():
    parser = argparse.ArgumentParser(description="Build and run the Kryta servers.")
    parser.add_argument("--root", default=os.path.dirname(os.path.abspath(__file__)),
                        help="the repository root (default: the folder this script is in)")
    options = parser.parse_args()
    root = os.path.abspath(options.root)
    if not os.path.exists(os.path.join(root, "Cargo.toml")):
        sys.exit("no Cargo.toml in %s; pass --root" % root)
    window = tk.Tk()
    try:
        ttk.Style().theme_use("vista" if IS_WINDOWS else "clam")
    except tk.TclError:
        pass
    Launcher(window, root)
    window.mainloop()


if __name__ == "__main__":
    main()
