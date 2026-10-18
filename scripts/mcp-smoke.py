#!/usr/bin/env python3
"""Scripted end-to-end client; no built-in LLM and no direct project-file writes.
Run: uv run --with 'mcp<2' python scripts/mcp-smoke.py --session ... --video a.mp4 --video b.mp4 --audio song.wav --output new.mp4
"""
import argparse
import asyncio
import importlib.metadata
import json
from pathlib import Path
from uuid import uuid4
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

async def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--session',required=True)
    parser.add_argument('--video',action='append',required=True)
    parser.add_argument('--audio',required=True)
    parser.add_argument('--output',required=True)
    args=parser.parse_args()
    if len(args.video)!=2:
        parser.error('Provide exactly two --video arguments.')
    root=Path(__file__).resolve().parents[1]
    server=StdioServerParameters(command=str(root/'target/debug/blyacut-mcp'),args=[args.session])
    async with stdio_client(server) as (read,write):
        async with ClientSession(read,write) as client:
            await client.initialize()
            tools=await client.list_tools()
            assert len(tools.tools)==12
            print(f'MCP SDK {importlib.metadata.version("mcp")}: initialize and tools/list passed',flush=True)
            async def call(name,arguments=None):
                result=await client.call_tool(name,arguments or {})
                if result.isError:
                    raise RuntimeError(f'{name}: {result.content}')
                structured=getattr(result,'structuredContent',None)
                return structured if structured is not None else json.loads(result.content[0].text)
            snapshot=await call('get_project')
            if any(t['clips'] for t in snapshot['project']['tracks']):
                raise RuntimeError('Demo requires an empty timeline; it will not remove existing edits.')
            assets=[]
            for path in [*args.video,args.audio]:
                before={a['id'] for a in snapshot['project']['assets']}
                snapshot=await call('import_media',{'path':str(Path(path).resolve())})
                assets.append(next(a for a in snapshot['project']['assets'] if a['id'] not in before))
            video_track=next(t['id'] for t in snapshot['project']['tracks'] if t['kind']=='video')
            audio_track=next((t['id'] for t in snapshot['project']['tracks'] if t['kind']=='audio'),str(uuid4()))
            edits=[]
            if not any(t['id']==audio_track for t in snapshot['project']['tracks']):
                edits.append({'operation':'add_track','track_id':audio_track,'name':'Music','kind':'audio'})
            length=0
            for index,asset in enumerate(assets[:2]):
                clip=str(uuid4());duration=min(2000,asset['duration_ms'])
                start=0 if index==0 else asset['duration_ms']-duration
                edits.extend([
                    {'operation':'append','asset_id':asset['id'],'track_id':video_track,'clip_id':clip},
                    {'operation':'trim_clip','clip_id':clip,'start_ms':start,'end_ms':start+duration},
                    {'operation':'set_audio','clip_id':clip,'gain_milli':200,'fade_in_ms':0,'fade_out_ms':0},
                ])
                length+=duration
            song=assets[2];clip=str(uuid4());duration=min(length,song['duration_ms']);fade=min(500,duration//2)
            edits.extend([
                {'operation':'append','asset_id':song['id'],'track_id':audio_track,'clip_id':clip},
                {'operation':'trim_clip','clip_id':clip,'start_ms':0,'end_ms':duration},
                {'operation':'set_audio','clip_id':clip,'gain_milli':250,'fade_in_ms':fade,'fade_out_ms':fade},
            ])
            snapshot=await call('apply_edits',{'expected_revision':snapshot['revision'],'edits':edits})
            print(f'Applied {len(edits)} edits as one action; live revision {snapshot["revision"]}',flush=True)
            job=await call('export_video',{'expected_revision':snapshot['revision'],'output_path':str(Path(args.output).resolve())})
            for _ in range(600):
                current=(await call('get_session'))['render_job']
                if current['id']!=job['id']:
                    raise RuntimeError('Unexpected replacement render job')
                if current['status']!='rendering':
                    if current['status']!='complete':
                        raise RuntimeError(current['error'])
                    print(f'Export complete: {args.output}',flush=True)
                    return
                await asyncio.sleep(0.2)
            raise TimeoutError('Export did not finish within 120 seconds')

if __name__=='__main__':
    asyncio.run(main())
