#!/usr/bin/env python3
"""Run local Phase 1 regressions only. No Rust, Windows, account or provider calls."""
from pathlib import Path
import argparse, datetime, json, platform, shutil, sqlite3, sys, time, unittest
ROOT=Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,default=ROOT/'audit/evidence/phase1-tests.json');args=p.parse_args()
 suite=unittest.defaultTestLoader.discover(str(ROOT/'audit/tests'),pattern='test_*.py')
 def names(s):
  for t in s:
   if isinstance(t,unittest.TestSuite):yield from names(t)
   else:yield t.id()
 ids=list(names(suite));args.output.parent.mkdir(parents=True,exist_ok=True);start=time.monotonic()
 with args.output.with_suffix('.log').open('w') as log:r=unittest.TextTestRunner(stream=log,verbosity=2).run(suite)
 report={'scope':'phase1_implementation_audit','at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'tests_run':r.testsRun,'passed':r.testsRun-len(r.failures)-len(r.errors)-len(r.skipped),'failures':len(r.failures),'errors':len(r.errors),'skipped':len(r.skipped),'elapsed_seconds':round(time.monotonic()-start,3),'groups':{k:sum('.'+k+'.' in i for i in ids) for k in ['GitBehavior','SqliteBehavior','ReferenceModels','SourceWiring']},'test_ids':ids,'failure_details':[{'id':t.id(),'trace':s} for t,s in r.failures+r.errors],'host':{'os':platform.system(),'python':platform.python_version(),'sqlite':sqlite3.sqlite_version,'rustc':shutil.which('rustc'),'cargo':shutil.which('cargo'),'powershell':shutil.which('pwsh')},'rust_tests_executed':0,'windows_tests_executed':0,'remote_device_calls':0,'live_provider_calls':0,'limitations':['Actual SQL/Git recipes are exercised on disposable Linux fixtures, not through compiled Rust.','ReferenceModels tests independently model intended algorithms; they do not execute Rust.','SourceWiring tests are static textual checks, not parser/typecheck or security proof.','Windows, compiler, networking/credentials and provider certification remain deferred.']}
 args.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report[k] for k in ['tests_run','passed','failures','errors','skipped','groups','elapsed_seconds']},indent=2));return not r.wasSuccessful()
if __name__=='__main__':sys.exit(main())
