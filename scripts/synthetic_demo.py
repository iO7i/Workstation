#!/usr/bin/env python3
"""Generate a synthetic illustration by executing shipped SQL + the independent Python economics oracle.
NOT a Rust application run, NOT a provider/agent integration, and NOT a production dataset.
"""
import html,json,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'verification'))
from test_database import db,workspace,work,session,assign,decision,accept,active
from reference_economics import forecast

def main():
 c=db();workspace(c);work(c,state='paused',ws='ws');session(c,'codex-ended',ws='ws');c.execute("UPDATE sessions SET ended_at=101 WHERE id='codex-ended'")
 session(c,'grok-reported',ws='ws');assign(c,s='grok-reported',lease=150)
 decision(c,'initial');accept(c,'initial');decision(c,'superseding','initial',effective=200,recorded=300);accept(c,'superseding',300)
 c.execute("INSERT INTO environments VALUES('p','dev')")
 r={'name':'Reserved example endpoint','locator':'https://example.org','environment':'dev','source_kind':'user_approved','provider_verified':False}
 c.execute("INSERT INTO resources VALUES('resource','p','dev','endpoint',?,100)",(json.dumps(r),))
 vectors=json.loads((ROOT/'fixtures/control/economics-vectors.json').read_text());normal=vectors[0]
 # Only benchmark the exact shipped temporal SQL query, not cached Rust context latency.
 durations=[]
 for _ in range(100):
  start=time.perf_counter();active(c,250,350);durations.append((time.perf_counter()-start)*1000)
 ordered=sorted(durations)
 result={'provenance':{'data':'synthetic','execution':'Python SQLite + independent reference oracle','rust_application_executed':False,'real_agents_launched':False,'providers_contacted':False},
 'modules':7,'work':{'state':'paused','primary_session':'grok-reported','source':'explicit synthetic insertion','lease_at_time_200':'ownership_uncertain_not_abandoned'},
 'chronicle':{'decision_valid_150_known_150':active(c,150,150),'decision_valid_250_known_250':active(c,250,250),'decision_valid_250_known_350':active(c,250,350),'explanation':'Retrospective acceptance cannot rewrite what was known before it was recorded.'},
 'atlas':{'dev':[r],'prod':[],'environment_fallback':False},
 'economics_reference':forecast(normal['samples'],normal['now']),
 'capabilities':{'catalog_entry_count':len(json.loads((ROOT/'catalog/resources.json').read_text())),'catalog_review':'purpose only','certified_integrations':0},
 'continuity':{'mode':'packet_only_source_implemented','real_cross_agent_handoff':'not_executed'},
 'sql_integrity':c.execute('PRAGMA integrity_check').fetchone()[0],
 'performance_reference_only':{'operation':'exact temporal decision SQL on a tiny synthetic database','samples':100,'p95_ms':ordered[94],'median_ms':ordered[49],'not_native_context_or_mcp_performance':True}}
 out=ROOT/'examples';out.mkdir(exist_ok=True);(out/'synthetic-demo.json').write_text(json.dumps(result,indent=2)+'\n')
 body=html.escape(json.dumps(result,indent=2))
 (out/'synthetic-demo.html').write_text('<!doctype html><html lang="en"><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'unsafe-inline\'; base-uri \'none\'; form-action \'none\'"><title>Workstation synthetic demonstration</title><style>body{font:16px system-ui;max-width:1100px;margin:40px auto;padding:20px}pre{white-space:pre-wrap;overflow-wrap:anywhere}</style><h1>Workstation — synthetic illustration</h1><p><strong>Not output from a compiled Rust app.</strong> Actual shipped SQL and an independent Python reference were exercised against synthetic inputs. No providers or agents were contacted.</p><pre>'+body+'</pre></html>')
 c.close();print(json.dumps({'written':['examples/synthetic-demo.json','examples/synthetic-demo.html'],'sql_integrity':result['sql_integrity'],'rust_executed':False,'measured_sql_only_p95_ms':ordered[94]},indent=2))
if __name__=='__main__':main()
