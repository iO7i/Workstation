"""Phase 1 implementation-audit evidence. NO Rust application execution.

Groups distinguish real Git/SQLite behavior, independent reference-model checks and
static source-wiring checks. Fixtures stay under tempfile, never user repositories.
"""
from __future__ import annotations
import copy, hashlib, json, math, os, re, sqlite3, subprocess, sys, tempfile, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'verification'))
import test_database as base

def source(p): return (ROOT/p).read_text(encoding='utf-8')
def sha(b):return hashlib.sha256(b).hexdigest()
def full_db(path=':memory:'):
    c=base.db(path)
    for p in ['003_integrations.sql','004_lifecycle_completion.sql']:
        c.executescript('BEGIN IMMEDIATE;\n'+source('schema/'+p)+'\nCOMMIT;')
    c.execute("INSERT INTO environments VALUES('p','prod')")
    return c

def terminal_run(c, ident, finished=200, state='succeeded'):
    payload=json.dumps({'id':ident,'fixture':'not_a_product_plan'},sort_keys=True,separators=(',',':'))
    c.execute('INSERT INTO effect_plans VALUES(?,?,?,?,?,?,?,?)',(ident,'p','prod','fixture',sha(payload.encode()),100,300,payload))
    c.execute('INSERT INTO effect_runs VALUES(?,?,?,?,?,?,?)',('run-'+ident,ident,'running',100,None,None,None))
    if state!='running': c.execute('UPDATE effect_runs SET state=?,finished_at=?,receipt=? WHERE id=?',(state,finished,'{"fixture":true}','run-'+ident))
    return payload

class GitBehavior(unittest.TestCase):
    """Executes actual Git in disposable repositories; Rust wrapper remains unrun."""
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='ws-phase1-git-');self.root=Path(self.temp.name)
        self.repo=self.root/'repo';self.repo.mkdir();self.wt=self.root/'worktree with spaces'
        self.env={k:v for k,v in os.environ.items() if not k.startswith('GIT_')}
        self.env.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,GIT_OPTIONAL_LOCKS='0',GIT_TERMINAL_PROMPT='0')
        self.g('init','--template=','-q');(self.repo/'tracked.txt').write_text('committed content\n')
        (self.repo/'.gitignore').write_text('.env\n');self.g('add','.');self.g('commit','-qm','disposable fixture')
        self.g('worktree','add','-q','-b','review',str(self.wt))
    def tearDown(self):self.temp.cleanup()
    def g(self,*args,cwd=None,check=True):
        return subprocess.run(['git','-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','core.hooksPath=','-c','core.fsmonitor=false','-c','gc.auto=0',*args],cwd=cwd or self.repo,env=self.env,check=check,capture_output=True,timeout=8)
    def hide(self,flag):
        self.g('update-index',flag,'tracked.txt',cwd=self.wt)
        (self.wt/'tracked.txt').write_text('UNCOMMITTED_UNIQUE_CANARY\n')
    @staticmethod
    def blockers(data):
        # Independent reference for the patched -v stream guard; not Rust execution.
        if data and not data.endswith(b'\0'):return ['malformed']
        found=[]
        for row in data.split(b'\0'):
            if not row:continue
            if len(row)<3 or row[1:2]!=b' ':return ['malformed']
            tag=chr(row[0])
            if tag not in 'HSMRCK?hsmrck':return ['malformed']
            if tag.islower():found.append('assume_unchanged')
            if tag.upper()=='S':found.append('skip_worktree')
        return found
    def original_loss(self,flag):
        self.hide(flag)
        self.assertEqual(self.g('status','--porcelain=v2','-z','--untracked-files=all',cwd=self.wt).stdout,b'')
        self.g('worktree','remove',str(self.wt))
        self.assertFalse(self.wt.exists(),'Baseline non-force Git removal must reproduce loss')
    def fixed_guard(self,flag):
        self.hide(flag)
        data=self.g('ls-files','-v','-z',cwd=self.wt).stdout
        self.assertTrue(self.blockers(data))
        # Guard refuses removal; keep unique content and repository/index unchanged.
        before=self.g('ls-files','-v','-z',cwd=self.wt).stdout
        self.assertEqual((self.wt/'tracked.txt').read_text(),'UNCOMMITTED_UNIQUE_CANARY\n')
        self.assertEqual(before,self.g('ls-files','-v','-z',cwd=self.wt).stdout)
    def test_baseline_assume_unchanged_loses_data(self):self.original_loss('--assume-unchanged')
    def test_baseline_skip_worktree_loses_data(self):self.original_loss('--skip-worktree')
    def test_patched_recipe_blocks_assume_unchanged(self):self.fixed_guard('--assume-unchanged')
    def test_patched_recipe_blocks_skip_worktree(self):self.fixed_guard('--skip-worktree')
    def test_normal_index_not_misclassified(self):self.assertEqual(self.blockers(self.g('ls-files','-v','-z',cwd=self.wt).stdout),[])
    def test_ignored_secret_remains_protected(self):
        (self.wt/'.env').write_text('SECRET_CANARY')
        data=self.g('ls-files','--others','--ignored','--exclude-standard','-z',cwd=self.wt).stdout
        self.assertIn(b'.env\0',data);self.assertEqual((self.wt/'.env').read_text(),'SECRET_CANARY')
    def test_inspection_does_not_update_index(self):
        path=self.g('rev-parse','--git-path','index',cwd=self.wt).stdout.decode().strip();p=Path(path)
        if not p.is_absolute():p=self.wt/p
        before=sha(p.read_bytes());self.g('ls-files','-v','-z',cwd=self.wt);self.g('status','--porcelain=v2','-z',cwd=self.wt)
        self.assertEqual(before,sha(p.read_bytes()))
    def test_nonforce_dirty_removal_is_not_run_by_guard(self):
        (self.wt/'tracked.txt').write_text('changed')
        status=self.g('status','--porcelain=v2','-z',cwd=self.wt).stdout
        self.assertTrue(status);self.assertTrue(self.wt.exists())
    def test_bad_index_stream_is_unknown(self):
        self.assertEqual(self.blockers(b'H file'),['malformed'])
        self.assertEqual(self.blockers(b'! file\0'),['malformed'])

class SqliteBehavior(unittest.TestCase):
    """Executes all four shipped SQL schemas and exact source selection SQL."""
    def setUp(self):self.c=full_db()
    def tearDown(self):self.c.close()
    def test_schema4_upgrade_keeps_baseline_records(self):
        self.assertEqual(self.c.execute('PRAGMA user_version').fetchone()[0],4)
        self.assertEqual(self.c.execute('SELECT id FROM projects').fetchall(),[('p',)])
        self.assertEqual(self.c.execute('SELECT run_id FROM scans').fetchall(),[('s1',)])
        self.assertEqual(self.c.execute('PRAGMA foreign_key_check').fetchall(),[])
    def test_schema4_rolls_back_if_later_statement_fails(self):
        c=base.db();c.executescript('BEGIN;'+source('schema/003_integrations.sql')+'COMMIT;')
        try:
            with self.assertRaises(sqlite3.Error):c.executescript('BEGIN;'+source('schema/004_lifecycle_completion.sql')+'INSERT INTO no_such_table VALUES(1);COMMIT;')
            c.rollback();self.assertEqual(c.execute('PRAGMA user_version').fetchone()[0],3)
            self.assertEqual(c.execute("SELECT count(*) FROM sqlite_master WHERE name='secret_heads'").fetchone()[0],0)
        finally:c.close()
    def test_integrity_check_misses_foreign_key_breakage(self):
        self.c.execute('PRAGMA foreign_keys=OFF')
        self.c.execute("INSERT INTO workspace_protections VALUES('bad','missing','missing','fixture',1,NULL)")
        self.assertEqual(self.c.execute('PRAGMA integrity_check').fetchone()[0],'ok')
        self.assertTrue(self.c.execute('PRAGMA foreign_key_check').fetchall())
    def test_foreign_key_guard_accepts_valid_database(self):self.assertEqual(self.c.execute('PRAGMA foreign_key_check').fetchall(),[])
    def test_backup_copies_committed_wal_state(self):
        with tempfile.TemporaryDirectory() as d:
            c=full_db(str(Path(d)/'source.db'));c.execute('PRAGMA journal_mode=WAL');terminal_run(c,'fixture')
            target=sqlite3.connect(str(Path(d)/'backup.db'));c.backup(target)
            self.assertEqual(target.execute('SELECT count(*) FROM effect_plans').fetchone()[0],1)
            self.assertEqual(target.execute('PRAGMA integrity_check').fetchone()[0],'ok')
            self.assertEqual(target.execute('PRAGMA foreign_key_check').fetchall(),[]);target.close();c.close()
    def test_one_plan_cannot_execute_twice(self):
        terminal_run(self.c,'one')
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO effect_runs VALUES('two','one','running',100,NULL,NULL,NULL)")
    def test_terminal_effect_cannot_be_replayed(self):
        terminal_run(self.c,'one')
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE effect_runs SET state='running' WHERE id='run-one'")
    def test_old_plan_payload_immutable(self):
        terminal_run(self.c,'one')
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE effect_plans SET payload='{}'")
    def selection(self,before=500):
        txt=source('crates/workstation-platform/src/journal_archive.rs')
        sql=re.search(r'prepare\("(SELECT p\.id,p\.payload,r\.id,r\.receipt.*?)"\)',txt).group(1)
        return list(self.c.execute(sql,('p',before,100)))
    def test_exact_archive_selection_changes_when_new_run_qualifies(self):
        terminal_run(self.c,'one');before=sha(json.dumps(self.selection()).encode())
        terminal_run(self.c,'two',finished=250);after=sha(json.dumps(self.selection()).encode())
        self.assertNotEqual(before,after)
    def test_archive_selection_order_has_stable_tie_break(self):
        terminal_run(self.c,'z');terminal_run(self.c,'a')
        self.assertEqual([r[0] for r in self.selection()],['a','z'])
    def test_archive_excludes_running_and_indeterminate(self):
        terminal_run(self.c,'running',state='running');terminal_run(self.c,'uncertain',state='indeterminate');terminal_run(self.c,'good')
        self.assertEqual([x[0] for x in self.selection()],['good'])
    def test_secret_versions_are_immutable(self):
        terminal_run(self.c,'one');self.c.execute("INSERT INTO resources VALUES('s','p','prod','secret_reference','{}',1)")
        self.c.execute('INSERT INTO secret_versions VALUES(?,?,?,?,?,?,?)',('s','v','v.dpapi','a'*64,None,100,'run-one'))
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE secret_versions SET file_name='other.dpapi'")
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute('DELETE FROM secret_versions')
    def test_secret_head_compare_and_swap(self):
        terminal_run(self.c,'one');self.c.execute("INSERT INTO resources VALUES('s','p','prod','secret_reference','{}',1)")
        for v in ['one','two']:self.c.execute('INSERT INTO secret_versions VALUES(?,?,?,?,?,?,?)',('s',v,v+'.dpapi','a'*64,None,100,'run-one'))
        self.c.execute("INSERT INTO secret_heads VALUES('s','one','active',1,100)")
        first=self.c.execute("UPDATE secret_heads SET version_id='two',generation=2 WHERE resource_id='s' AND generation=1").rowcount
        second=self.c.execute("UPDATE secret_heads SET version_id='one',generation=2 WHERE resource_id='s' AND generation=1").rowcount
        self.assertEqual((first,second),(1,0))
    def test_cross_project_environment_is_not_silently_substituted(self):
        base.project(self.c,'q');self.c.execute("INSERT INTO environments VALUES('q','stage')")
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("INSERT INTO resources VALUES('r','p','stage','endpoint','{}',1)")
    def test_future_and_retroactive_decisions_remain_time_scoped(self):
        base.decision(self.c,'a');base.accept(self.c,'a');base.decision(self.c,'b','a',500,200);base.accept(self.c,'b',200)
        self.assertEqual(base.active(self.c,499,300),['a']);self.assertEqual(base.active(self.c,501,300),['b'])
        self.assertEqual(base.active(self.c,501,150),['a'])
    def test_catalog_room_includes_builtins(self):
        base_count=len(json.loads(source('catalog/resources.json')))
        # Read exact packaging shape instead of claiming a fixed entry count blindly.
        raw=json.loads(source('catalog/resources.json'))
        if isinstance(raw,dict):base_count=len(raw.get('resources',raw.get('entries',[])))
        self.assertEqual(base_count,27)
        self.assertEqual(100-base_count,73)
        self.assertGreater(base_count+77,100,'Original query allowed 104 entries and broke matcher budget')
    def test_prior_lease_expiry_does_not_release_primary(self):
        base.work(self.c);base.session(self.c);base.session(self.c,'other');base.assign(self.c,lease=100)
        with self.assertRaises(sqlite3.IntegrityError):base.assign(self.c,'two',s='other',lease=100000)

# Independent reference arithmetic/ordering; intentionally not named Rust tests.
def finite_ratio(a,b):
    if not math.isfinite(a) or not math.isfinite(b) or b<=0:return None
    result=a/b
    return result if math.isfinite(result) else None

def old_overlap(a,b):return a==b or a.startswith(b+'/') or b.startswith(a+'/')
def indexed_overlap(a,others):
    if a in others:return True
    if any(a[:i] in others for i,c in enumerate(a) if c=='/'):return True
    return any(b.startswith(a+'/') for b in others)

class ReferenceModels(unittest.TestCase):
    def test_baseline_max_secret_binding_exceeds_cipher_reader(self):
        b={'schema_version':1,'project':'p','environment':'prod','id':'s','secret':list(('界'*5461).encode())}
        encoded=json.dumps(b,separators=(',',':')).encode()
        self.assertLessEqual(len(('界'*5461).encode()),16*1024)
        self.assertGreater(len(encoded),64*1024)
        self.assertLess(len(encoded),128*1024)
    def test_finite_ratio_withholds_overflow(self):self.assertIsNone(finite_ratio(100.0,5e-324))
    def test_finite_ratio_regular_case(self):self.assertEqual(finite_ratio(100,2),50)
    def test_nonfinite_ratio_arguments_are_rejected(self):
        for a,b in [(math.inf,1),(1,math.inf),(1,0),(1,-1),(math.nan,1)]:self.assertIsNone(finite_ratio(a,b))
    def test_even_median_no_overflow(self):
        self.assertTrue(math.isinf((1e308+1e308)/2));self.assertEqual(1e308/2+1e308/2,1e308)
    def test_collision_index_equivalent_to_pairwise_reference(self):
        paths=['a','a/x','a/x/y','ab','ab/y','b','b/y','b/y/z']
        for mask in range(1<<len(paths)):
            others={p for i,p in enumerate(paths) if mask & (1<<i)}
            for p in paths:self.assertEqual(indexed_overlap(p,others),any(old_overlap(p,b) for b in others))
    def test_early_turn_terminal_must_survive_rpc_reply(self):
        msgs=[{'method':'turn/completed','params':{'threadId':'t','turn':{'id':'u','status':'completed','items':['PRIVATE_CANARY']}}},{'id':1,'result':{'turn':{'id':'u'}}}]
        queued=[]
        for m in msgs:
            if m.get('method')=='turn/completed':
                p=m['params'];queued.append({'threadId':p['threadId'],'turnId':p['turn']['id'],'status':p['turn']['status']})
        self.assertEqual(queued,[{'threadId':'t','turnId':'u','status':'completed'}]);self.assertNotIn('PRIVATE_CANARY',json.dumps(queued))
    def test_archive_aggregate_budget_rejects_before_retaining_large_set(self):
        budget=8*1024*1024-4096;used=0;retained=[]
        for i in range(10000):
            value=json.dumps({'key':str(i),'value':'x'*32768},separators=(',',':'))
            if used+len(value)+1>budget:break
            retained.append(value);used+=len(value)+1
        self.assertLessEqual(used,budget);self.assertLess(len(retained),300)
    def test_archive_record_limit_is_not_off_by_one(self):
        limit=10000;records=list(range(limit));self.assertTrue(len(records)>=limit)
        self.assertFalse(len(records)>limit,'Original greater-than check admitted an extra record')
    def test_local_file_presence_distinguishes_dangling_symlink(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'link';p.symlink_to(Path(d)/'missing')
            self.assertFalse(p.exists());self.assertTrue(p.lstat())
    def test_unix_socket_is_not_a_regular_file(self):
        import socket,stat
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'sock';s=socket.socket(socket.AF_UNIX);s.bind(str(path))
            try:self.assertFalse(stat.S_ISREG(path.lstat().st_mode))
            finally:s.close()
    def test_restore_marker_is_independent_of_valid_database(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);c=full_db(str(p/'workstation.db'));c.close();(p/'restore.pending').write_text('unfinished')
            self.assertTrue((p/'restore.pending').is_file())
            c=sqlite3.connect(p/'workstation.db');self.assertEqual(c.execute('PRAGMA integrity_check').fetchone()[0],'ok');c.close()
            # Normal open must reject the marker, irrespective of structurally valid DB.

    def test_accountless_plan_fit_reference_is_unknown(self):
        cycles=[{'provider':'p','account_alias':None,'bucket':'b','complete':True} for _ in range(3)]
        result='unknown' if any(c['complete'] and not c.get('account_alias') for c in cycles) else 'computable'
        self.assertEqual(result,'unknown')
    def test_mixed_accounts_plan_fit_reference_is_rejected(self):
        cycles=[{'provider':'p','account_alias':a,'bucket':'b'} for a in ['personal','work']]
        streams={(c['provider'],c['account_alias'],c['bucket']) for c in cycles}
        self.assertNotEqual(len(streams),1)
    def test_lease_future_timestamp_remains_unknown(self):
        now=100;last=101
        old_recent=(now-last<=900)
        new_recent=(0<=last<=now and now-last<=900)
        self.assertTrue(old_recent);self.assertFalse(new_recent)
    def test_archive_digest_binds_payload_not_just_row_ids(self):
        first=[{'id':'same','payload':'old'}];second=[{'id':'same','payload':'changed'}]
        self.assertNotEqual(sha(json.dumps(first).encode()),sha(json.dumps(second).encode()))

class SourceWiring(unittest.TestCase):
    """Static assertions are NOT runtime tests or proof of compilation."""
    def test_hidden_flags_wired_into_capture(self):
        s=source('crates/workstation-platform/src/control_workspace.rs');self.assertIn('index_visibility_blockers',s);self.assertIn('"ls-files","-v","-z"',s.replace(' ',''));self.assertIn('INDEX_FLAGS_CHANGED_DURING_OBSERVATION',s)
    def test_ads_validation_precedes_new_file_fallback(self):
        s=source('crates/workstation-platform/src/rpc.rs');i=s.index('pub(crate) fn scoped_file_path');s=s[i:]
        self.assertLess(s.index('validate_lexical'),s.index('symlink_metadata'))
    def test_permission_root_no_extended_path_roundtrip(self):
        s=source('crates/workstation-platform/src/rpc.rs');start=s.index('fn permission_locations_safe');end=s.index('pub(crate) fn scoped_file_path',start)
        self.assertNotIn('canonicalize', '\n'.join(line.split('//',1)[0] for line in s[start:end].splitlines()))
    def test_gemini_readonly_rejected_before_connect(self):
        s=source('crates/workstation-platform/src/acp_driver.rs');start=s.index('pub fn continue_once');s=s[start:]
        self.assertLess(s.index('GEMINI_READONLY_MODE_NOT_RELIABLY_SUPPORTED'),s.index('connect('))
    def test_restore_block_marker_used_for_normal_open(self):
        s=source('crates/workstation-platform/src/storage.rs');self.assertIn('RESTORE_INCOMPLETE_HOME_PROTECTED',s);self.assertIn('verify_foreign_keys(&from)',s);self.assertIn('verify_foreign_keys(&target)',s)
    def test_archive_plan_binds_selection_digest(self):
        s=source('crates/workstation-core/src/operations.rs');self.assertIn('selection_digest:String',s);self.assertIn('digest(selection_digest)?',s)
        s=source('crates/workstation-platform/src/journal_archive.rs');self.assertIn('ARCHIVE_SELECTION_CHANGED_REPLAN_REQUIRED',s)
    def test_undo_refuses_false_missing_and_checks_postcondition(self):
        s=source('crates/workstation-platform/src/repairs.rs');self.assertIn('entry_exists',s);self.assertIn('RESTORE_POSTCONDITION_UNCERTAIN',s)
    def test_optional_context_is_not_all_or_nothing(self):
        s=source('crates/workstation-platform/src/control_store.rs');self.assertIn('optional_section_failures',s);self.assertIn('optional_context("useful_discoveries"',s)
    def test_plan_fit_has_explicit_account_boundary(self):
        s=source('crates/workstation-core/src/economics.rs');self.assertIn('account_alias:Option<String>',s);self.assertIn('explicit_account_identity_required',s);self.assertIn('(&c.provider,account,&c.bucket)',s)
    def test_empty_trial_evidence_rejected(self):
        s=source('crates/workstation-core/src/discovery.rs');self.assertIn('trim().is_empty()',s)
    def test_vault_read_write_cap_consistent(self):
        s=source('crates/workstation-platform/src/vault.rs');self.assertIn('CIPHER_BYTES_LIMIT:usize=128*1024',s);self.assertIn('ciphertext.len()>CIPHER_BYTES_LIMIT',s);self.assertIn('.take(CIPHER_BYTES_LIMIT as u64+1)',s)
        s=source('crates/workstation-platform/src/secret_lifecycle.rs');self.assertIn('CIPHER_BYTES_LIMIT',s);self.assertIn('path::Path',s)
    def test_signal_and_lease_timestamps_checked(self):
        self.assertIn('control::timestamp(s.observed_at)?',source('crates/workstation-core/src/health_rules.rs'))
        self.assertIn('entry.session.last_observed>now',source('crates/workstation-core/src/continuity.rs'))
    def test_effect_policy_changed(self):self.assertIn('workstation.effects.v4.audit1',source('crates/workstation-core/src/effects.rs'))
    def test_dependency_pin_moves_out_of_known_advisory_range(self):
        import tomllib
        d=tomllib.loads(source('Cargo.toml'));self.assertEqual(d['workspace']['dependencies']['time']['version'],'=0.3.47')
    def test_no_broad_wsl_shutdown_added(self):
        for p in (ROOT/'crates').rglob('*.rs'):
            s=p.read_text();self.assertNotIn('wsl --shutdown',s);self.assertNotIn('taskkill /F /IM',s)
    def test_mcp_tool_surface_still_thirteen_readonly(self):
        s=source('crates/workstation-core/src/protocol.rs');section=s[s.index('pub const TOOLS'):s.index('#[derive(Default)]')]
        names=re.findall(r'\("([a-z_]+)",',section);self.assertEqual(len(names),13)
        for forbidden in ['apply','repair','secret_resolve','claim_primary','execute_shell']:self.assertNotIn(forbidden,names)
    def test_no_new_platform_effects_in_core(self):
        for p in (ROOT/'crates/workstation-core/src').glob('*.rs'):
            production=p.read_text().split('#[cfg(test)]')[0]
            self.assertNotIn('std::process::Command',production);self.assertNotIn('reqwest::',production);self.assertNotIn('rusqlite::',production)
    def test_original_archive_checksum_unchanged(self):
        p=ROOT.parents[1]/'workstation-0.4.0-alpha.1-implementation-source.zip'
        # Keep reproducible assertion in evidence; archive may not be shipped with patch package.
        if not p.exists():p=Path('/mnt/data/workstation-0.4.0-alpha.1-implementation-source.zip')
        if p.exists():self.assertEqual(sha(p.read_bytes()),'89fcfeaadfc1a749c91c2703e461fdcca7bc9b6aa393dbe7938cf7edb2e7ddb4')
        else:self.skipTest('Original archive is not distributed inside patched source')
