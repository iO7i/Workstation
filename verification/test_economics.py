"""Tests independent numeric oracle and shared synthetic vectors, NOT compiled Rust."""
import copy,json,math,unittest
from pathlib import Path
from reference_economics import forecast,pareto
ROOT=Path(__file__).resolve().parents[1]
VECTORS=json.loads((ROOT/'fixtures/control/economics-vectors.json').read_text())
class EconomicsReferenceTests(unittest.TestCase):
 def test_linear_closed_form(self):
  v=VECTORS[0];r=forecast(v['samples'],v['now']);self.assertAlmostEqual(r['horizons'][-1]['exhaustion_after_seconds'],(100-20)/(10/3600))
 def test_recent_burst_rate_exceeds_long_baseline(self):
  a=copy.deepcopy(VECTORS[0]);last=copy.deepcopy(a['samples'][-1]);last.update(id='new',observed_at=4300,used=30);r=forecast(a['samples']+[last],4300)
  self.assertGreater(r['horizons'][0]['rate_per_second'],r['horizons'][3]['rate_per_second'])
 def test_no_eta_beyond_reset(self):
  a=copy.deepcopy(VECTORS[0]);
  for s in a['samples']:s['reset_at']=4000
  r=forecast(a['samples'],3700);self.assertIsNone(r['horizons'][-1]['exhaustion_after_seconds']);self.assertGreater(r['horizons'][-1]['capacity_at_current_pace_seconds'],300)
 def test_rate_scaling_gives_inverse_capacity_duration(self):
  a=copy.deepcopy(VECTORS[0]);b=copy.deepcopy(a)
  for s in b['samples']:s['used']*=2;s['limit']*=2
  self.assertAlmostEqual(forecast(a['samples'],a['now'])['horizons'][-1]['exhaustion_after_seconds'],forecast(b['samples'],b['now'])['horizons'][-1]['exhaustion_after_seconds'])
 def test_no_cross_provider_sum(self):
  a=copy.deepcopy(VECTORS[0]);a['samples'][0]['provider']='other'
  with self.assertRaisesRegex(ValueError,'MIXED_STREAMS'):forecast(a['samples'],a['now'])
 def test_unknown_capacity_stays_unknown(self):
  a=copy.deepcopy(VECTORS[0]);
  for s in a['samples']:s['limit']=None
  self.assertIsNone(forecast(a['samples'],a['now'])['remaining'])
 def test_pareto_removes_dominated_not_all_costly_models(self):
  rows=json.loads((ROOT/'fixtures/control/models.json').read_text());out=pareto(rows);self.assertNotIn('dominated',out);self.assertIn('quality',out)
 def test_benchmark_revision_mixing_rejected(self):
  rows=json.loads((ROOT/'fixtures/control/models.json').read_text());rows[1]['revision']='other'
  with self.assertRaises(ValueError):pareto(rows)
 def test_benchmark_license_unknown_rejected(self):
  rows=json.loads((ROOT/'fixtures/control/models.json').read_text());rows[1]['license_state']='unknown'
  with self.assertRaises(ValueError):pareto(rows)
 def test_declared_unavailable_model_filtered(self):
  rows=json.loads((ROOT/'fixtures/control/models.json').read_text());rows[0]['available']=False;self.assertNotIn('balanced',pareto(rows))

def vector_test(vector):
 def run(self):
  self.assertTrue(vector['synthetic'])
  expected=vector['expected']
  if 'error' in expected:
   with self.assertRaisesRegex(ValueError,expected['error']):forecast(vector['samples'],vector['now'])
   return
  result=forecast(vector['samples'],vector['now'])
  if 'remaining' in expected:self.assertEqual(result['remaining'],expected['remaining'])
  if 'horizons' in expected:self.assertEqual(len(result['horizons']),expected['horizons'])
  if 'confidence' in expected:self.assertEqual(result['confidence'],expected['confidence'])
  if 'blended_seconds' in expected:self.assertAlmostEqual(result['horizons'][-1]['exhaustion_after_seconds'],expected['blended_seconds'])
  if expected.get('all_eta_null'):self.assertTrue(all(h['exhaustion_after_seconds'] is None for h in result['horizons']))
 return run
for vector in VECTORS:setattr(EconomicsReferenceTests,'test_vector_'+vector['name'].replace('-','_'),vector_test(vector))
if __name__=='__main__':unittest.main(verbosity=2)
