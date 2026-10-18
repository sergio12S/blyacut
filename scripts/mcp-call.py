#!/usr/bin/env python3
"""Call one BlyaCut tool with the official MCP SDK; arguments are a JSON object.
Usage: uv run --with 'mcp<2' python scripts/mcp-call.py SESSION TOOL '[JSON]'
"""
import asyncio
import json
import sys
from pathlib import Path
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

async def main():
    if len(sys.argv) not in (3,4):
        raise SystemExit(__doc__)
    session_path,tool=sys.argv[1:3]
    arguments=json.loads(sys.argv[3]) if len(sys.argv)==4 else {}
    root=Path(__file__).resolve().parents[1]
    async with stdio_client(StdioServerParameters(command=str(root/'target/debug/blyacut-mcp'),args=[session_path])) as (read,write):
        async with ClientSession(read,write) as client:
            await client.initialize()
            result=await client.call_tool(tool,arguments)
            print(result.model_dump_json(indent=2))
            if result.isError:
                raise SystemExit(1)

if __name__=='__main__':
    asyncio.run(main())
