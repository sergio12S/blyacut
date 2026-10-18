#!/usr/bin/env python3
"""Run an agent script against ONE BlyaCut MCP session (official MCP SDK).

The script is plain Python with `call(tool, args={})` (returns the tool's JSON, raises on
tool errors) and `log(*values)` in scope. Use it when your agent has no MCP client of its own.

Usage:
  uv run --with 'mcp<2' python scripts/mcp-script.py --headless DIR [--autonomous] SCRIPT.py
  uv run --with 'mcp<2' python scripts/mcp-script.py --session SESSION.json SCRIPT.py
"""
import argparse
import asyncio
import json
import queue
import threading
from pathlib import Path
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

class ToolError(RuntimeError):
    pass

async def main():
    parser = argparse.ArgumentParser(usage=__doc__)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument("--headless", metavar="DIR")
    target.add_argument("--session", metavar="SESSION_JSON")
    parser.add_argument("--autonomous", action="store_true")
    parser.add_argument("script")
    a = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    args = ["--headless", a.headless] + (["--autonomous"] if a.autonomous else []) if a.headless else [a.session]
    server = StdioServerParameters(command=str(root / "target/debug/blyacut-mcp"), args=args)
    async with stdio_client(server) as (read, write):
        async with ClientSession(read, write) as client:
            await client.initialize()
            loop = asyncio.get_running_loop()
            requests: "queue.Queue" = queue.Queue()

            def call(tool, arguments=None):
                done = threading.Event()
                box = {}
                requests.put((tool, arguments or {}, box, done))
                done.wait()
                if "error" in box:
                    raise ToolError(f"{tool}: {box['error']}")
                return box["value"]

            def run_script():
                scope = {"call": call, "log": print, "json": json, "ToolError": ToolError}
                try:
                    exec(compile(Path(a.script).read_text(), a.script, "exec"), scope)
                finally:
                    requests.put(None)

            worker = threading.Thread(target=run_script)
            worker.start()
            while True:
                item = await loop.run_in_executor(None, requests.get)
                if item is None:
                    break
                tool, arguments, box, done = item
                result = await client.call_tool(tool, arguments)
                text = result.content[0].text if result.content else "null"
                if result.isError:
                    box["error"] = text
                else:
                    box["value"] = result.structuredContent if result.structuredContent is not None else json.loads(text)
                done.set()
            worker.join()

if __name__ == "__main__":
    asyncio.run(main())
