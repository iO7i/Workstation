#!/usr/bin/env python3
"""Execute shipped SQL/Git recipe/schema tests and independent synthetic economics oracle.
This does NOT compile or run Rust, PowerShell, DPAPI, MCP, or vendor applications.
"""
import argparse,contextlib,datetime,importlib.metadata,json,platform,shutil,sqlite3,subprocess,sys,time,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ROOT/'evidence/control-tests.json');args=parser.parse_args()
 sys.path.insert(0,str(ROOT/'verification'));suite=unittest.defaultTestLoader.discover(str(ROOT/'verification'),pattern='test_*.py')
 def flatten(x):
  for test in x:
   if isinstance(test,unittest.TestSuite):yield from flatten(test)
   else:yield test.id()
 ids=list(flatten(suite));args.output.parent.mkdir(parents=True,exist_ok=True);log=args.output.with_suffix('.log')
 start=time.perf_counter()
 with log.open('w') as stream:result=unittest.TextTestRunner(stream=stream,verbosity=2).run(suite)
 report={'scope':'slice2_independent_test_host_checks','executed_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'tests_run':result.testsRun,'passed':result.testsRun-len(result.failures)-len(result.errors)-len(result.skipped),'failed':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'elapsed_seconds':round(time.perf_counter()-start,4),
 'groups':{name:sum(name in i for i in ids) for name in ['test_database','test_git_recipes','test_economics','test_contracts']},'test_ids':ids,
 'host':{'platform':platform.system(),'python':platform.python_version(),'sqlite':sqlite3.sqlite_version,'jsonschema':importlib.metadata.version('jsonschema'),'git':subprocess.check_output(['git','--version'],text=True).strip() if shutil.which('git') else None,'rustc':shutil.which('rustc'),'cargo':shutil.which('cargo'),'powershell':shutil.which('pwsh')},
 'rust_tests_executed':0,'windows_tests_executed':0,'live_providers_exercised':[],
 'boundaries':['SQL tests execute shipped schema and current-decision query through Python SQLite.','Git tests execute bounded recipes on disposable Linux repositories, not the Rust supervisor.','Economics reference tests execute a Python oracle and shared synthetic vectors, not Rust.','Schema/catalog/source-inspection checks do not prove native compilation or integration safety.'],
 'failures':[{'test':t.id(),'trace':trace} for t,trace in result.failures+result.errors]}
 args.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report[k] for k in ['tests_run','passed','failed','errors','skipped','groups','elapsed_seconds','host']},indent=2));return 0 if result.wasSuccessful() else 1
if __name__=='__main__':raise SystemExit(main())
