# cps — Cudane Python Subsystem default theme
# Cyan accent, host-aware, multi-line capable.

ACCENT = "#22d3ee"
CWD_COLOR = "#a78bfa"
DIM = "#6b7280"
SUCCESS = "#22c55e"
ERROR = "#ef4444"
BRAND = "#0ea5e9"


def render_prompt(**context):
    cwd = context.get("cwd", "~")
    user = context.get("user", "")
    host = context.get("host", "")
    exit_code = int(context.get("exit_code", "0") or "0")
    brand = context.get("brand", "cps")

    status = "\u2714" if exit_code == 0 else "\u2718"

    lines_above = []
    if user and host:
        lines_above.append(f"[{brand}] {user}@{host}")

    return {
        "lines_above": lines_above,
        "input_prefix": f"{cwd} {status} \u276f ",
        "right_prompt": "",
        "colors": {
            "accent": ACCENT,
            "cwd": CWD_COLOR,
            "dim": DIM,
            "success": SUCCESS,
            "error": ERROR,
            "brand": BRAND,
        },
    }


def render_right_prompt(**context):
    return context.get("brand", "cps")


def render_command_summary(**context):
    code = int(context.get("exit_code", "0") or "0")
    if code == 0:
        return "\u2714 ok"
    return f"\u2718 exit {code}"


def run():
    """Full-screen mode: a tiny animated demo driven by cps."""
    import time

    try:
        import shutil
        cols = shutil.get_terminal_size((80, 24)).columns
    except Exception:
        cols = 80

    frame = 0
    try:
        while True:
            bar = ("#" * (frame % cols)).ljust(cols)
            print(f"\r[{bar}] cps", end="", flush=True)
            frame += 1
            time.sleep(0.05)
    except KeyboardInterrupt:
        print()
        return True
