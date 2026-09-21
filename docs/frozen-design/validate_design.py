"""Validate design artifacts only. Does not implement or certify Workstation."""
from pathlib import Path
import copy, json, re, sqlite3
from jsonschema import Draft202012Validator, FormatChecker, ValidationError
P=Path(__file__).resolve().parent
results=[]
def record(name, fn):
    fn(); results.append(name)

def reject(fn, exc):
    try: fn()
    except exc: return
    raise AssertionError('Expected rejection did not occur')

schemas={}
for path in sorted((P/'contracts').glob('*.schema.json')):
    name=path.name.removesuffix('.schema.json')
    schema=json.loads(path.read_text())
    Draft202012Validator.check_schema(schema)
    schemas[name]=Draft202012Validator(schema,format_checker=FormatChecker())
    example=json.loads((P/'examples'/f'{name}.json').read_text())
    record(f'contract positive: {name}',lambda n=name,e=example:schemas[n].validate(e))

negative_specs=[
 ('action-plan','arbitrary shell operation',lambda x:x['actions'][0].update(kind='execute_shell')),
 ('action-plan','unbounded attempts',lambda x:x.update(max_attempts=100)),
 ('action-plan','no approval',lambda x:x.update(approval_required=False)),
 ('action-plan','unknown evidence permits action',lambda x:x.update(on_unknown='apply')),
 ('action-plan','repair grants network',lambda x:x.update(network_allowed=True)),
 ('secret-reference','extra plaintext value field',lambda x:x.update(value='SYNTHETIC-NOT-A-SECRET')),
 ('secret-reference','agent reveal flag',lambda x:x.update(expose_to_agent=True)),
 ('project-manifest','executable manifest field',lambda x:x.update(command='not-an-allowed-field')),
 ('observation','fake GREEN coverage',lambda x:x.update(coverage='green')),
 ('fingerprint','inline shell repair',lambda x:x.update(shell='not-allowed')),
]
for name,label,mutate in negative_specs:
    example=json.loads((P/'examples'/f'{name}.json').read_text()); mutate(example)
    record(f'contract negative: {label}',lambda n=name,e=example:reject(lambda:schemas[n].validate(e),ValidationError))

con=sqlite3.connect(':memory:')
record('SQL: initialize full reference schema',lambda:con.executescript((P/'schema'/'001_initial.sql').read_text()))
assert con.execute('PRAGMA foreign_keys').fetchone()[0]==1
results.append('SQL: foreign keys enabled')
con.execute("INSERT INTO projects VALUES ('p','demo','Demo','2026-09-18T00:00:00Z',NULL)")
con.execute("INSERT INTO projects VALUES ('q','other','Other','2026-09-18T00:00:00Z',NULL)")
con.execute("INSERT INTO environments VALUES ('dev','p','Development','development')")
record('SQL: foreign key rejects unknown project',lambda:reject(lambda:con.execute("INSERT INTO environments VALUES ('bad','missing','Bad','other')"),sqlite3.IntegrityError))
record('SQL: cross-project environment reference rejected',lambda:reject(lambda:con.execute("INSERT INTO resources VALUES ('r','q','dev','endpoint','Label',NULL,NULL,NULL,'{}','proposed',NULL,'2026-09-18T00:00:00Z')"),sqlite3.IntegrityError))
record('SQL: malformed JSON rejected',lambda:reject(lambda:con.execute("INSERT INTO capability_connections VALUES ('c','cap',NULL,'proposed','{invalid',NULL,'2026-09-18T00:00:00Z')"),sqlite3.IntegrityError))
record('SQL: secret expose-to-agent rejected',lambda:reject(lambda:con.execute("INSERT INTO secret_refs VALUES ('s','p','dev','local_dpapi',NULL,'{}','{}',1)"),sqlite3.IntegrityError))

# Explicit column list prevents positional drift in the test fixture.
def insert_decision(i,eff,rec,pred):
    con.execute('INSERT INTO decisions(id,project_id,topic_key,scope_key,scope_json,statement,rationale,alternatives_json,consequences_json,author,effective_at,recorded_at,predecessor_id) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)',
       (i,'p','storage','dev','{"environment_id":"dev"}',i+' statement','rationale','[]','[]','user',eff,rec,pred))
insert_decision('d1','2026-09-01T00:00:00Z','2026-09-01T00:00:00Z',None)
insert_decision('d2','2026-09-20T00:00:00Z','2026-09-18T00:00:00Z','d1')
record('SQL: decision revision immutable',lambda:reject(lambda:con.execute("UPDATE decisions SET statement='changed' WHERE id='d1'"),sqlite3.IntegrityError))
record('SQL: decision deletion rejected',lambda:reject(lambda:con.execute("DELETE FROM decisions WHERE id='d1'"),sqlite3.IntegrityError))
con.execute("INSERT INTO decision_transitions VALUES ('t1','d1','accepted','user','2026-09-01T00:00:00Z','2026-09-01T00:00:00Z',NULL,'approval1')")
con.execute("INSERT INTO decision_transitions VALUES ('t2','d2','accepted','user','2026-09-20T00:00:00Z','2026-09-18T00:00:00Z',NULL,'approval2')")
con.execute("INSERT INTO decision_transitions VALUES ('t3','d1','superseded','user','2026-09-20T00:00:00Z','2026-09-18T00:00:00Z','d2',NULL)")
record('SQL: acceptance requires approval reference',lambda:reject(lambda:con.execute("INSERT INTO decision_transitions VALUES ('bad','d1','accepted','user','2026-09-01T00:00:00Z','2026-09-01T00:00:00Z',NULL,NULL)"),sqlite3.IntegrityError))
record('SQL: transition immutable',lambda:reject(lambda:con.execute("UPDATE decision_transitions SET action='rejected' WHERE id='t1'"),sqlite3.IntegrityError))
con.execute("INSERT INTO audit_events VALUES ('a1','test','user','2026-09-18T00:00:00Z',NULL,'{}')")
record('SQL: audit immutable',lambda:reject(lambda:con.execute("UPDATE audit_events SET event_type='changed' WHERE id='a1'"),sqlite3.IntegrityError))

def query_at(effective,known):
    return [r[0] for r in con.execute('''SELECT d.id FROM decisions d JOIN decision_transitions a ON d.id=a.decision_id
    WHERE d.project_id='p' AND d.topic_key='storage' AND d.scope_key='dev' AND a.action='accepted'
    AND a.effective_at<=? AND a.recorded_at<=? AND NOT EXISTS (
      SELECT 1 FROM decision_transitions s WHERE s.decision_id=d.id
      AND s.action IN ('withdrawn','superseded') AND s.effective_at<=? AND s.recorded_at<=?) ORDER BY d.id''',(effective,known,effective,known))]

def expect(actual,expected):
    assert actual==expected,(actual,expected)
record('SQL example: future acceptance does not replace current decision early',lambda:expect(query_at('2026-09-18T12:00:00Z','2026-09-18T12:00:00Z'),['d1']))
record('SQL example: replacement applies at effective boundary',lambda:expect(query_at('2026-09-21T00:00:00Z','2026-09-21T00:00:00Z'),['d2']))
record('SQL example: earlier knowledge does not include later-recorded replacement',lambda:expect(query_at('2026-09-21T00:00:00Z','2026-09-17T00:00:00Z'),['d1']))
record('SQL: database integrity check',lambda:expect(con.execute('PRAGMA integrity_check').fetchone()[0],'ok'))
record('SQL: foreign-key check',lambda:expect(con.execute('PRAGMA foreign_key_check').fetchall(),[]))

sources=json.loads((P/'sources.json').read_text())
ids={x['id'] for x in sources}
refs=set(re.findall(r'\[(S\d+)\]',(P/'SYSTEM-DESIGN.md').read_text()))
record('Sources: all master references resolve',lambda:expect(refs-ids,set()))
record('Sources: IDs unique',lambda:expect(len(ids),len(sources)))
record('Artifacts: every schema has a synthetic example',lambda:expect(set(schemas),{x.stem for x in (P/'examples').glob('*.json')}))

report={'status':'artifact_checks_passed','checks':len(results),'sqlite_runtime_used_for_schema_tests':sqlite3.sqlite_version,'schema_examples':len(schemas),'primary_sources':len(sources),'results':results,'not_tested':['Windows executable','Windows process ownership','DPAPI','Docker recovery','agent integrations','provider access','real backups/restores','62 implementation acceptance cases']}
(P/'validation-results.json').write_text(json.dumps(report,indent=2)+'\n')
(P/'VALIDATION.md').write_text(f'''# Design-pack validation

**Result:** {len(results)} artifact checks passed. These validate the design artifacts, not the proposed product.

Checked {len(schemas)} JSON Schemas and synthetic examples, rejection of selected disallowed fields/actions, executable SQLite schema creation, relational constraints, immutable record guards, three illustrative temporal queries, source-reference consistency, and example completeness. Detailed results: `validation-results.json`. Reproduce with `python validate_design.py` after installing the Python `jsonschema` package in a separate test environment.

The SQL was exercised in an in-memory database using the container's SQLite **{sqlite3.sqlite_version}**. That test does **not** certify this runtime for production, enable WAL, or satisfy the frozen patched-SQLite release requirement. Application release validation must check its actual bundled SQLite version/source ID separately.

The schemas validate structure, not confidentiality or authority. Rejecting a `value` field does not detect every secret that could be embedded in another string. The temporal queries demonstrate the reference ledger shape, not a completed conflict resolver or authorized decision service.

## Not executed here

No Windows application was compiled or run. No actual process was inspected or stopped, no Docker/WSL runtime repaired, no Codex state edited, no Git worktree removed, no DPAPI encryption/decryption performed, no credential accessed, and no vendor/provider integration certified. The 62 cases in `ACCEPTANCE-MATRIX.md` remain implementation requirements, not reported passes.

The master design is the frozen build specification. The SQL and contracts are reference starting points; the builder must implement the domain invariants, security gates, capability tests, and real Windows acceptance evidence before release.
''')
print(json.dumps({'checks':len(results),'sqlite':sqlite3.sqlite_version,'schemas':len(schemas),'status':'passed'},indent=2))
