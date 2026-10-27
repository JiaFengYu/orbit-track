#!/usr/bin/env python3
"""Measure warm loopback HTTP including propagation, serialization and transport."""
import http.client,json,pathlib,platform,statistics,time
client=http.client.HTTPConnection('127.0.0.1',8788,timeout=10)
url='/v1/satellites/25544/track?start=epoch&minutes=90&step_seconds=30'
results=[]
for i in range(210):
    start=time.perf_counter();client.request('GET',url);r=client.getresponse();body=r.read();elapsed=(time.perf_counter()-start)*1000
    assert r.status==200
    assert len(json.loads(body)['samples'])==181
    if i>=10:results.append(elapsed)
cpu=next(x.split(':',1)[1].strip() for x in pathlib.Path('/proc/cpuinfo').read_text().splitlines() if x.startswith('model name'))
print(json.dumps({'platform':platform.system()+' '+platform.machine(),'cpu':cpu,'warm_requests':200,'samples':181,'http_mean_ms':statistics.mean(results),'http_median_ms':statistics.median(results),'http_p95_ms':sorted(results)[189],'scope':'release server, warm loopback HTTP, includes propagation+conversion+serialization+transport, excludes fetch and JSON decoding'},indent=2))
