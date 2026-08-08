# cps — minimal theme
# Single-line prompt with exit-code indicator.

ACCENT = "#7dd3fc"
DIM = "#525252"


def render_prompt(**context):
    cwd = context.get("cwd", "~")
    exit_code = int(context.get("exit_code", "0") or "0")
    mark = "" if exit_code == 0 else "?"
    return {
        "lines_above": [],
        "input_prefix": f"{cwd}{mark} $ ",
        "right_prompt": "",
        "colors": {
            "accent": ACCENT,
            "dim": DIM,
        },
    }


def render_right_prompt(**context):
    return ""


def render_command_summary(**context):
    return ""
