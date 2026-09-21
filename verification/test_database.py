"""Executes the SHIPPED SQL and exact current-decision query, not Rust application code."""
import json, sqlite3, tempfile, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
SCHEMA1=(ROOT/'schema/001_r0.sql').read_text();SCHEMA2=(ROOT/'schema/002_control_plane.sql').read_text()
CURRENT=(ROOT/'schema/current_decisions.sql').read_text()
def db(path=':memory:',upgrade=True):
 c=sqlite3.connect(path,isolation_level=None,timeout=.1);c.execute('PRAGMA foreign_keys=ON');c.executescript(SCHEMA1)
 c.execute('INSERT INTO projects VALUES(?,?,?)',('p','/synthetic/project','/synthetic/git'))
 c.execute('INSERT INTO roots VALUES(?,?,?)',('root','/synthetic/root','workspace'))
 c.execute('INSERT INTO scans VALUES(?,?,?)',('s1','2026-09-18T00:00:00Z','{"source":"synthetic"}'))
 if upgrade:c.executescript('BEGIN IMMEDIATE;\n'+SCHEMA2+'\nCOMMIT;')
 return c
def project(c,p):c.execute('INSERT INTO projects VALUES(?,?,?)',(p,'/synthetic/'+p,'/synthetic/git'))
def workspace(c,w='ws',p='p'):c.execute('INSERT INTO workspace_registrations VALUES(?,?,?,?)',(p,w,'/synthetic/'+p+'/'+w,'volume:fileid'))
def work(c,w='w',p='p',state='planned',version=0,ws=None):
 payload=json.dumps(dict(id=w,project_id=p,state=state,version=version,workspace_id=ws))
 c.execute('INSERT INTO work_items VALUES(?,?,?,?,?,?,?)',(w,p,state,version,ws,payload,100))
def session(c,s='session',p='p',ws=None):c.execute('INSERT INTO sessions VALUES(?,?,?,?,?,?,?,?)',(s,p,'synthetic','opaque-'+s,ws,100,None,'{}'))
def assign(c,i='a',w='w',p='p',s='session',role='primary',lease=200):c.execute('INSERT INTO assignments VALUES(?,?,?,?,?,?,?,?)',(i,w,p,s,role,lease,100,None))
def decision(c,d,pre=None,effective=100,recorded=100,p='p',topic='architecture',scope='all'):
 payload=json.dumps(dict(id=d,project_id=p,topic=topic,scope=scope,predecessor=pre,effective_at=effective,recorded_at=recorded,statement='synthetic decision '+d))
 c.execute('INSERT INTO decisions VALUES(?,?,?,?,?,?,?,?)',(d,p,topic,scope,pre,effective,recorded,payload))
def accept(c,d,at=100):
 row=c.execute('SELECT project_id,topic,scope,predecessor FROM decisions WHERE id=?',(d,)).fetchone()
 c.execute('INSERT INTO decision_acceptances VALUES(?,?,?,?,?,?,?)',(d,*row,at,'local-fixture-approval'))
def active(c,valid=1000,known=1000):return [json.loads(x[0])['id'] for x in c.execute(CURRENT,('p',valid,known))]
class DatabaseTests(unittest.TestCase):
 def setUp(self):self.c=db()
 def tearDown(self):self.c.close()
 def test_upgrade_preserves_r0_data(self):
  self.assertEqual(self.c.execute('SELECT id FROM projects').fetchall(),[('p',)])
  self.assertEqual(self.c.execute('SELECT run_id FROM scans').fetchall(),[('s1',)])
  self.assertEqual(self.c.execute('SELECT id FROM roots').fetchall(),[('root',)])
 def test_schema_version_and_application_id(self):
  self.assertEqual(self.c.execute('PRAGMA user_version').fetchone()[0],2);self.assertEqual(self.c.execute('PRAGMA application_id').fetchone()[0],1465078832)
 def test_foreign_keys_and_integrity(self):self.assertEqual(self.c.execute('PRAGMA foreign_key_check').fetchall(),[]);self.assertEqual(self.c.execute('PRAGMA integrity_check').fetchone()[0],'ok')
 def test_future_projects_get_profile(self):project(self.c,'new');self.assertEqual(self.c.execute("SELECT economics_mode FROM project_profiles WHERE project_id='new'").fetchone()[0],'balanced')
 def test_invalid_economic_mode(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE project_profiles SET economics_mode='auto_upgrade_plan'")
 def test_rollback_incomplete_upgrade(self):
  c=db(upgrade=False)
  try:
   with self.assertRaises(sqlite3.OperationalError):c.executescript('BEGIN IMMEDIATE;'+SCHEMA2+"INSERT INTO does_not_exist VALUES(1);COMMIT;")
   c.rollback();self.assertEqual(c.execute('PRAGMA user_version').fetchone()[0],1);self.assertEqual(c.execute("SELECT count(*) FROM sqlite_master WHERE name='work_items'").fetchone()[0],0)
  finally:c.close()
 def test_rerun_migration_not_silent(self):
  with self.assertRaises(sqlite3.OperationalError):self.c.executescript('BEGIN IMMEDIATE;'+SCHEMA2+'COMMIT;')
  self.c.rollback();self.assertEqual(self.c.execute('PRAGMA user_version').fetchone()[0],2)
 def test_no_abandoned_work_state(self):
  with self.assertRaises(sqlite3.IntegrityError):work(self.c,state='abandoned')
 def test_work_payload_matches_columns(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute('INSERT INTO work_items VALUES(?,?,?,?,?,?,?)',('w','p','active',0,None,'{"state":"done","version":0}',100))
 def test_work_payload_requires_state(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute('INSERT INTO work_items VALUES(?,?,?,?,?,?,?)',('w','p','active',0,None,'{}',100))
 def test_active_cannot_skip_review_to_done(self):
  work(self.c,state='active')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE work_items SET state='done',payload=json_set(payload,'$.state','done') WHERE id='w'")
 def test_review_can_complete(self):
  work(self.c,state='review');self.c.execute("UPDATE work_items SET state='done',payload=json_set(payload,'$.state','done')");self.assertEqual(self.c.execute('SELECT state FROM work_items').fetchone()[0],'done')
 def test_done_cannot_resume(self):
  work(self.c,state='done')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE work_items SET state='active',payload=json_set(payload,'$.state','active')")
 def test_optimistic_work_version(self):
  work(self.c);first=self.c.execute("UPDATE work_items SET version=1,payload=json_set(payload,'$.version',1) WHERE id='w' AND version=0").rowcount
  second=self.c.execute("UPDATE work_items SET version=1,payload=json_set(payload,'$.version',1) WHERE id='w' AND version=0").rowcount
  self.assertEqual((first,second),(1,0))
 def test_work_cannot_reference_other_project_workspace(self):
  project(self.c,'q');workspace(self.c,p='q')
  with self.assertRaises(sqlite3.IntegrityError):work(self.c,ws='ws')
 def test_session_cannot_reference_other_workspace(self):
  project(self.c,'q');workspace(self.c,p='q')
  with self.assertRaises(sqlite3.IntegrityError):session(self.c,ws='ws')
 def test_one_primary(self):
  work(self.c);session(self.c);session(self.c,'s2');assign(self.c)
  with self.assertRaises(sqlite3.IntegrityError):assign(self.c,'a2',s='s2')
 def test_expiry_does_not_release_primary(self):
  work(self.c);session(self.c);session(self.c,'s2');assign(self.c,lease=100)
  with self.assertRaises(sqlite3.IntegrityError):assign(self.c,'a2',s='s2',lease=1000000)
 def test_explicit_release_allows_new_primary(self):
  work(self.c);session(self.c);session(self.c,'s2');assign(self.c);self.c.execute("UPDATE assignments SET released_at=201 WHERE id='a'");assign(self.c,'a2',s='s2',lease=300)
  self.assertEqual(self.c.execute("SELECT count(*) FROM assignments WHERE role='primary' AND released_at IS NULL").fetchone()[0],1)
 def test_reviewer_not_second_primary(self):
  work(self.c);session(self.c);session(self.c,'s2');assign(self.c);assign(self.c,'a2',s='s2',role='reviewer');self.assertEqual(self.c.execute('SELECT count(*) FROM assignments').fetchone()[0],2)
 def test_cross_project_assignment_rejected(self):
  work(self.c);project(self.c,'q');session(self.c,p='q')
  with self.assertRaises(sqlite3.IntegrityError):assign(self.c)
 def test_session_external_id_stays_opaque(self):
  session(self.c);self.c.execute("UPDATE sessions SET external_id='opaque/not-our-id?x=1'");self.assertEqual(self.c.execute('SELECT external_id FROM sessions').fetchone()[0],'opaque/not-our-id?x=1')
 def test_future_successor_does_not_erase_current(self):
  decision(self.c,'a');accept(self.c,'a');decision(self.c,'b','a',effective=500,recorded=200);accept(self.c,'b',200)
  self.assertEqual(active(self.c,499,300),['a']);self.assertEqual(active(self.c,500,500),['b'])
 def test_retroactive_record_does_not_change_past_knowledge(self):
  decision(self.c,'a');accept(self.c,'a');decision(self.c,'b','a',effective=150,recorded=500);accept(self.c,'b',500)
  self.assertEqual(active(self.c,200,200),['a']);self.assertEqual(active(self.c,200,500),['b'])
 def test_unaccepted_proposal_not_authority(self):decision(self.c,'a');self.assertEqual(active(self.c),[])
 def test_incompatible_roots_conflict(self):
  decision(self.c,'a');accept(self.c,'a');decision(self.c,'other')
  with self.assertRaises(sqlite3.IntegrityError):accept(self.c,'other')
 def test_incompatible_successors_conflict(self):
  decision(self.c,'a');accept(self.c,'a');decision(self.c,'b','a',200,200);accept(self.c,'b',200);decision(self.c,'c','a',200,200)
  with self.assertRaises(sqlite3.IntegrityError):accept(self.c,'c',200)
 def test_cross_scope_supersession_rejected(self):
  decision(self.c,'a')
  with self.assertRaises(sqlite3.IntegrityError):decision(self.c,'b','a',topic='different')
 def test_acceptance_precedes_record_rejected(self):
  decision(self.c,'a',recorded=500)
  with self.assertRaises(sqlite3.IntegrityError):accept(self.c,'a',100)
 def test_successor_knowledge_not_before_parent(self):
  decision(self.c,'a');accept(self.c,'a',500);decision(self.c,'b','a',effective=200,recorded=200)
  with self.assertRaises(sqlite3.IntegrityError):accept(self.c,'b',300)
 def test_decisions_are_immutable(self):
  decision(self.c,'a')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE decisions SET payload='{}'")
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute('DELETE FROM decisions')
 def test_accepted_history_is_immutable(self):
  decision(self.c,'a');accept(self.c,'a')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute('DELETE FROM decision_acceptances')
 def test_rejected_proposal_not_later_accepted(self):
  decision(self.c,'a');self.c.execute("INSERT INTO decision_dispositions VALUES('a','rejected',101,'approval')")
  with self.assertRaises(sqlite3.IntegrityError):accept(self.c,'a',102)
 def test_accepted_decision_requires_supersession(self):
  decision(self.c,'a');accept(self.c,'a')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO decision_dispositions VALUES('a','withdrawn',101,'approval')")
 def test_resource_requires_explicit_environment(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO resources VALUES('r','p','prod','endpoint','{}',100)")
 def test_prod_never_falls_back_to_dev(self):
  self.c.execute("INSERT INTO environments VALUES('p','dev')");self.c.execute("INSERT INTO resources VALUES('r','p','dev','endpoint','{}',100)")
  self.assertEqual(self.c.execute("SELECT * FROM resources WHERE project_id='p' AND environment='prod'").fetchall(),[])
 def test_resource_claims_not_generic_green(self):
  self.c.execute("INSERT INTO environments VALUES('p','prod')");self.c.execute("INSERT INTO resources VALUES('r','p','prod','endpoint','{}',100)")
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO resource_verifications VALUES('v','r','fully_safe',100,'{}')")
 def test_checkpoint_immutable(self):
  workspace(self.c);work(self.c,ws='ws');self.c.execute("INSERT INTO checkpoints VALUES('cp','w','p',NULL,'ws',?,'digest',100,'{}')",('a'*40,))
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE checkpoints SET payload='{}'")
 def test_handoff_cross_project_rejected(self):
  workspace(self.c);work(self.c,ws='ws');self.c.execute("INSERT INTO checkpoints VALUES('cp','w','p',NULL,'ws',?,'digest',100,'{}')",('a'*40,));project(self.c,'q')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO handoffs VALUES('h','cp','q','grok','digest',NULL,100,'prepared')")
 def test_handoff_is_not_reported_native_resume(self):
  workspace(self.c);work(self.c,ws='ws');self.c.execute("INSERT INTO checkpoints VALUES('cp','w','p',NULL,'ws',?,'digest',100,'{}')",('a'*40,))
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO handoffs VALUES('h','cp','p','grok','digest',NULL,100,'native_resumed')")
 def test_quota_negative_rejected(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO quota_samples VALUES('q','p','test','account','bucket','subscription','percent','w',-1,100,1000,100,'{}')")
 def test_missing_quota_capacity_is_null(self):
  self.c.execute("INSERT INTO quota_samples VALUES('q','p','test','account','bucket','subscription','credits','w',1,NULL,1000,100,'{}')");self.assertIsNone(self.c.execute('SELECT capacity FROM quota_samples').fetchone()[0])
 def test_duplicate_quota_timestamp_rejected(self):
  sql="INSERT INTO quota_samples VALUES(?,'p','test','account','bucket','subscription','percent','w',1,100,1000,100,'{}')";self.c.execute(sql,('q',))
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute(sql,('other',))
 def test_three_economic_meters_are_separate(self):
  for i,meter in enumerate(['subscription','context','dollars']):self.c.execute("INSERT INTO quota_samples VALUES(?,'p','test','account','bucket',?,'unit','w',1,100,1000,100,'{}')",(str(i),meter))
  self.assertEqual(self.c.execute('SELECT count(DISTINCT meter) FROM quota_samples').fetchone()[0],3)
 def test_snooze_needs_actual_expiry(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO capability_feedback VALUES('f','p','resource','need','snoozed',NULL,'not_recorded',NULL,NULL,100)")
 def test_adoption_not_benefit(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO capability_feedback VALUES('f','p','resource','need','adopted',NULL,'useful','proof','revision',100)")
 def test_evaluated_usefulness_requires_provenance(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO capability_feedback VALUES('f','p','resource','need','evaluated',NULL,'useful',NULL,NULL,100)")
 def test_plan_maximum_expiry(self):
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO plans VALUES('plan','p','digest',100,5000,'v1','proposed','{}')")
 def test_approved_plan_cannot_change_target_payload(self):
  self.c.execute("INSERT INTO plans VALUES('plan','p','digest',100,200,'v1','blocked','{}')")
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE plans SET payload='{}'")
 def test_audit_is_append_only(self):
  self.c.execute("INSERT INTO audit_events(event_id,project_id,kind,recorded_at,source_kind,payload) VALUES('a','p','test',100,'observed','{}')")
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute('DELETE FROM audit_events')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE audit_events SET payload='{}'")
 def test_backup_restore_retains_decision_history(self):
  decision(self.c,'a');accept(self.c,'a');decision(self.c,'b','a',200,200);accept(self.c,'b',200)
  copy=sqlite3.connect(':memory:');self.c.backup(copy)
  try:self.assertEqual(active(copy),['b']);self.assertEqual(copy.execute('PRAGMA integrity_check').fetchone()[0],'ok')
  finally:copy.close()
 def test_readonly_connection_cannot_write(self):
  with tempfile.TemporaryDirectory() as t:
   p=Path(t)/'snapshot.db';copy=sqlite3.connect(p);self.c.backup(copy);copy.close();ro=sqlite3.connect(p.as_uri()+'?mode=ro',uri=True)
   try:
    with self.assertRaises(sqlite3.OperationalError):ro.execute("INSERT INTO environments VALUES('p','bad')")
   finally:ro.close()
 def test_quota_retention_cannot_delete_decisions(self):
  decision(self.c,'a');accept(self.c,'a');self.c.execute('DELETE FROM quota_samples');self.assertEqual(active(self.c),['a'])
 def test_concurrent_primary_claims_serialize(self):
  with tempfile.TemporaryDirectory() as t:
   p=Path(t)/'state.db';a=db(p);work(a);session(a);session(a,'s2');b=sqlite3.connect(p,isolation_level=None,timeout=.03);b.execute('PRAGMA foreign_keys=ON')
   try:
    a.execute('BEGIN IMMEDIATE');assign(a)
    with self.assertRaises(sqlite3.OperationalError):b.execute('BEGIN IMMEDIATE')
    a.commit()
    with self.assertRaises(sqlite3.IntegrityError):assign(b,'b',s='s2')
   finally:a.close();b.close()

 def test_workspace_observation_cannot_reference_other_project(self):
  project(self.c,'q');workspace(self.c,p='q')
  with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO workspace_observations VALUES('o','p','ws',100,'observed','{}')")
 def test_workspace_observation_keeps_protection_separate(self):
  workspace(self.c);self.c.execute("INSERT INTO workspace_observations VALUES('o','p','ws',100,'observed','{}')")
  self.assertEqual(self.c.execute('SELECT source_kind FROM workspace_observations').fetchone()[0],'observed')

if __name__=='__main__':unittest.main(verbosity=2)
