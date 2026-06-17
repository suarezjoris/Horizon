import sys
import json
import asyncio
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

async def run(cmd_args, method, params=None):
    if not cmd_args:
        print("Error: No command provided", file=sys.stderr)
        sys.exit(1)
        
    cmd = cmd_args[0]
    args = cmd_args[1:]
    
    server_params = StdioServerParameters(command=cmd, args=args)
    
    try:
        async with stdio_client(server_params) as (read, write):
            async with ClientSession(read, write) as session:
                await session.initialize()
                
                if method == "tools/list":
                    res = await session.list_tools()
                    tools = []
                    for t in res.tools:
                        tools.append({
                            "type": "function",
                            "function": {
                                "name": f"mcp_{t.name}",
                                "description": t.description or "",
                                "parameters": t.inputSchema
                            }
                        })
                    print(json.dumps(tools))
                    
                elif method == "tools/call":
                    if not params or "name" not in params:
                        print(json.dumps({"error": "Missing tool name"}))
                        return
                        
                    original_name = params["name"].removeprefix("mcp_")
                    res = await session.call_tool(original_name, params.get("arguments", {}))
                    
                    contents = []
                    for c in res.content:
                        if c.type == "text":
                            contents.append(c.text)
                        else:
                            contents.append(f"[{c.type} content]")
                    print(json.dumps(contents))
    except Exception as e:
        print(json.dumps({"error": str(e)}))

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python mcp_bridge.py <method> <params_json> <cmd> [args...]", file=sys.stderr)
        sys.exit(1)
        
    method = sys.argv[1]
    
    try:
        params = json.loads(sys.argv[2]) if sys.argv[2] != "null" else None
    except json.JSONDecodeError:
        params = None
        
    cmd_args = sys.argv[3:]
    
    asyncio.run(run(cmd_args, method, params))
