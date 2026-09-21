"""Real JSON Schema/catalog checks; do not execute the Rust serializer or MCP server."""
import copy,json,re,unittest
from pathlib import Path
from jsonschema import Draft202012Validator,ValidationError
ROOT=Path(__file__).resolve().parents[1]
def schema(name):return json.loads((ROOT/'contracts/control'/f'{name}.schema.json').read_text())
def validate(name,value):Draft202012Validator(schema(name)).validate(value)
class ContractTests(unittest.TestCase):
 def test_six_contracts_are_valid(self):
  paths=list((ROOT/'contracts/control').glob('*.schema.json'));self.assertEqual(len(paths),6)
  for p in paths:Draft202012Validator.check_schema(json.loads(p.read_text()))
 def test_metadata_variants_track_rust_enum(self):
  source=(ROOT/'crates/workstation-platform/src/control_store.rs').read_text().split('pub enum Record {')[1].split('\n}')[0]
  found={}
  for name,raw in re.findall(r'\s*(\w+)\{([^\n]+)\},?',source):found[re.sub(r'(?<!^)(?=[A-Z])','_',name).lower()]={p.split(':',1)[0] for p in raw.split(',')}
  actual={v['properties']['operation']['const']:set(v['properties'])-{'operation'} for v in schema('record')['oneOf']}
  self.assertEqual(found,actual)
 def test_valid_work_request(self):validate('record',{'operation':'work_create','id':'w','project_id':'p','objective':'synthetic','priority':2,'workspace_id':None})
 def test_no_abandoned_state(self):
  with self.assertRaises(ValidationError):validate('record',{'operation':'work_update','id':'w','expected_version':0,'state':'abandoned','completed':[],'remaining':[],'blockers':[]})
 def test_arbitrary_shell_not_a_record(self):
  with self.assertRaises(ValidationError):validate('record',{'operation':'run_shell','command':'echo no'})
 def test_extra_instruction_field_rejected(self):
  with self.assertRaises(ValidationError):validate('record',{'operation':'environment_add','project_id':'p','name':'prod','instructions':'do unsafe thing'})
 def test_candidate_cannot_self_approve(self):
  x={'title':'fixture','kind':'documentation','source':'https://example.org','needs':['parallel_work'],'purpose':'reference','limitations':['synthetic'],'review_status':'reviewed'}
  with self.assertRaises(ValidationError):validate('candidate-input',x)
 def test_reference_only_catalog(self):
  rows=json.loads((ROOT/'catalog/resources.json').read_text());self.assertEqual(len(rows),27)
  for row in rows:validate('capability',row);self.assertEqual(row['adoption_mode'],'reference_only')
 def test_catalog_has_unique_ids_and_sources(self):
  rows=json.loads((ROOT/'catalog/resources.json').read_text());self.assertEqual(len({r['id'] for r in rows}),len(rows));self.assertEqual(len({r['source'].rstrip('/') for r in rows}),len(rows))
 def test_no_catalog_installer_or_secret(self):
  rows=json.loads((ROOT/'catalog/resources.json').read_text())
  for row in rows:
   self.assertNotIn('command',row);self.assertNotIn('secret',row);self.assertTrue(row['limitations']);self.assertTrue(row['license_scope']);self.assertTrue(row['source'].startswith('https://'));self.assertNotIn('@',row['source'])
 def test_fixture_models_match_contract(self):
  for model in json.loads((ROOT/'fixtures/control/models.json').read_text()):validate('model-metric',model)
 def test_valid_economics_vectors_match_contract(self):
  vectors=json.loads((ROOT/'fixtures/control/economics-vectors.json').read_text())
  if isinstance(vectors,dict):vectors=vectors['cases']
  for case in vectors:
   if 'error' not in case.get('expected',{}):
    for row in case['samples']:validate('quota-sample',row)
 def test_negative_used_rejected(self):
  row=json.loads((ROOT/'fixtures/control/economics-vectors.json').read_text())[0]['samples'][0];row['used']=-1
  with self.assertRaises(ValidationError):validate('quota-sample',row)
 def test_no_custom_crypto_dependency(self):
  vault=(ROOT/'crates/workstation-platform/src/vault.rs').read_text();self.assertIn('CryptProtectData',vault);self.assertIn('CryptUnprotectData',vault)
 def test_mcp_declares_only_fixed_read_tools(self):
  src=(ROOT/'crates/workstation-core/src/protocol.rs').read_text();part=src.split('pub const TOOLS:')[1].split('];')[0];names=re.findall(r'\("([a-z_]+)",',part)
  self.assertEqual(len(names),13);self.assertFalse(set(names)&{'record','assign','secret_resolve','approve_plan','run','apply'})
 def test_mcp_handler_uses_readonly_connection(self):
  src=(ROOT/'crates/workstation-cli/src/mcp.rs').read_text();self.assertIn('Store::open_readonly',src);self.assertNotIn('store.record(',src)
 def test_new_action_operation_labels_present(self):
  src=(ROOT/'crates/workstation-cli/src/main.rs').read_text();part=src.split('fn operation(')[1].split('fn main()')[0]
  for name in ['SelectModels','DiagnoseEvidence','Upgrade','Restore','Record','Context','Timeline','Mcp','Handoff','VaultPut']:self.assertIn('Action::'+name,part)
 def test_archive_catalog_review_not_runtime_certification(self):
  rows=json.loads((ROOT/'catalog/REVIEW-REGISTER.json').read_text())
  self.assertEqual(rows['certified_integrations'],0);self.assertTrue(all(not x['runtime_tested'] and not x['license_audited'] for x in rows['sources']))
 def test_example_record_files_match_current_contract(self):
  for p in (ROOT/'examples/requests').glob('*.json'):validate('record',json.loads(p.read_text()))
 def test_example_checkpoint_matches_contract(self):validate('checkpoint-input',json.loads((ROOT/'examples/checkpoint.json').read_text()))
if __name__=='__main__':unittest.main()
