# cps example plugin
# Top-level callables are event hooks. Install via p.desc or `cps plugin register`.


def on_startup(**context):
    print(f"[example] startup with {len(context)} context keys")


def on_shutdown(**context):
    print("[example] shutdown")


def on_command(**context):
    cmd = context.get("command", "")
    if cmd:
        print(f"[example] ran: {cmd}")
