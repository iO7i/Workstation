"""Runs Git commands used by the source, on disposable repositories only. Not the Rust runner."""
import hashlib,os,shutil,subprocess,tempfile,unittest
from pathlib import Path
class GitRecipeTests(unittest.TestCase):
 def setUp(self):
  self.git=shutil.which('git');self.assertTrue(self.git,'Git required; do not silently skip this evidence')
  self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name);self.repo=self.root/'repo 日本語';self.wt=self.root/'worktree العربية';self.repo.mkdir()
  self.env={k:v for k,v in os.environ.items() if not k.startswith('GIT_')};self.env.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,GIT_TERMINAL_PROMPT='0',GIT_OPTIONAL_LOCKS='0',GIT_NO_LAZY_FETCH='1')
  self.g(self.repo,'init','--template=','-q');(self.repo/'tracked.txt').write_text('initial');(self.repo/'.gitignore').write_text('.env\n')
  self.g(self.repo,'add','.');self.g(self.repo,'commit','-q','-m','fixture')
  self.g(self.repo,'worktree','add','-q','-b','task',str(self.wt))
 def tearDown(self):self.temp.cleanup()
 def g(self,path,*args,check=True):
  return subprocess.run([self.git,'--no-pager','--no-optional-locks','-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','core.fsmonitor=false','-c','core.hooksPath=','-c','core.untrackedCache=false','-c','core.preloadIndex=false','-c','protocol.allow=never','-c','diff.external=','-c','submodule.recurse=false',*args],cwd=path,env=self.env,capture_output=True,check=check,timeout=5)
 def dirty(self):
  (self.wt/'tracked.txt').write_text('changed but not committed');(self.wt/'new.txt').write_text('untracked');(self.wt/'.env').write_text('SECRET_CANARY=private value')
 def status(self):return self.g(self.wt,'status','--porcelain=v1','-z','--untracked-files=all','--ignored=matching','--ignore-submodules=all').stdout
 def index(self):return Path(self.g(self.wt,'rev-parse','--path-format=absolute','--git-path','index').stdout.decode().strip())
 def test_unicode_worktree_membership(self):
  out=self.g(self.repo,'worktree','list','--porcelain','-z').stdout;self.assertIn(str(self.wt).encode(),out);self.assertTrue(out.endswith(b'\0\0'))
 def test_dirty_untracked_ignored_are_visible_without_values(self):
  self.dirty();out=self.status();self.assertIn(b' M tracked.txt\0',out);self.assertIn(b'?? new.txt\0',out);self.assertIn(b'!! .env\0',out);self.assertNotIn(b'SECRET_CANARY',out)
 def test_status_preserves_index_and_working_files(self):
  self.dirty();before={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in self.wt.iterdir() if p.is_file()};index=self.index().read_bytes();self.status()
  self.assertEqual(index,self.index().read_bytes());self.assertEqual(before,{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in self.wt.iterdir() if p.is_file()})
 def test_external_filter_detected_by_name_without_execution(self):
  marker=self.root/'should-not-exist';self.g(self.wt,'config','filter.unsafe.clean','echo SECRET_CANARY_FILTER > '+str(marker));self.g(self.wt,'config','filter.unsafe.process','other-command')
  out=self.g(self.wt,'config','--name-only','--get-regexp',r'^filter\..*\.(clean|process)$').stdout
  self.assertIn(b'filter.unsafe.clean',out);self.assertIn(b'filter.unsafe.process',out);self.assertNotIn(b'SECRET_CANARY_FILTER',out);self.assertFalse(marker.exists())
 def test_no_filter_query_exit_one_is_not_failure(self):
  out=self.g(self.wt,'config','--name-only','--get-regexp',r'^filter\..*\.(clean|process)$',check=False);self.assertEqual(out.returncode,1);self.assertEqual(out.stdout,b'')
 def test_local_only_commit_count_without_network(self):
  self.g(self.wt,'update-ref','refs/remotes/origin/main','HEAD');(self.wt/'tracked.txt').write_text('new commit');self.g(self.wt,'add','.');self.g(self.wt,'commit','-q','-m','local')
  count=int(self.g(self.wt,'rev-list','--count','HEAD','--not','--remotes').stdout);self.assertEqual(count,1)
 def test_stale_head_changes_after_commit(self):
  old=self.g(self.wt,'rev-parse','HEAD').stdout;(self.wt/'tracked.txt').write_text('new commit');self.g(self.wt,'add','.');self.g(self.wt,'commit','-q','-m','local');self.assertNotEqual(old,self.g(self.wt,'rev-parse','HEAD').stdout)
 def test_changed_metadata_changes_fingerprint_input(self):
  self.dirty();p=self.wt/'tracked.txt';before=(self.status(),p.stat().st_size,p.stat().st_mtime_ns);p.write_text('different size and metadata long text');after=(self.status(),p.stat().st_size,p.stat().st_mtime_ns);self.assertNotEqual(before,after)
 def test_metadata_only_blind_spot_is_explicit(self):
  self.dirty();p=self.wt/'tracked.txt';stat=p.stat();old=(self.status(),stat.st_size,stat.st_mtime_ns);p.write_text('x'*stat.st_size);os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns));new=(self.status(),p.stat().st_size,p.stat().st_mtime_ns)
  self.assertEqual(old,new,'Metadata is not byte identity: the recipient must inspect and test content.')
 def test_git_worktree_lock_is_visible(self):
  self.g(self.repo,'worktree','lock','--reason','active synthetic test',str(self.wt));self.assertIn(b'locked active synthetic test',self.g(self.repo,'worktree','list','--porcelain','-z').stdout)
 def test_git_operation_marker_path_resolves(self):
  path=Path(self.g(self.wt,'rev-parse','--path-format=absolute','--git-path','MERGE_HEAD').stdout.decode().strip());self.assertTrue(path.is_absolute());self.assertFalse(path.exists())
 def test_trusted_git_does_not_need_global_config(self):
  out=self.g(self.wt,'rev-parse','--verify','HEAD');self.assertEqual(len(out.stdout.strip()),40)
if __name__=='__main__':unittest.main(verbosity=2)
