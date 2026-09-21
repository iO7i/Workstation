#!/usr/bin/env python3
"""FS-1.0-D1 SPECIFICATION tests, not Rust/Windows/product certification.
Python 3.11+ and jsonschema in a separate development environment. No web requests.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import time
import unittest
from jsonschema import Draft202012Validator, FormatChecker, ValidationError

ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / 'specs/discovery/v1'
loader = importlib.util.spec_from_file_location('d1_policy_reference', SPEC / 'reference_policy.py')
policy = importlib.util.module_from_spec(loader)
loader.loader.exec_module(policy)
NOW = '2026-09-18T12:00:00Z'

def load(name): return json.loads((SPEC/'examples'/f'{name}.json').read_text(encoding='utf-8'))
def validator(name): return Draft202012Validator(json.loads((SPEC/'contracts'/f'{name}.schema.json').read_text(encoding='utf-8')), format_checker=FormatChecker())
def fresh_resource(id='example-interface-review', **updates):
    item = load('catalog-resource'); item['id'] = id
    if id != 'example-interface-review':
        item['canonical_url'] = f'https://example.org/workstation-fixtures/{id}'
        item['equivalence_key'] = None
    item.update(updates); return item

class DiscoverySpecificationTests(unittest.TestCase):
    def setUp(self):
        self.context = load('project-context'); self.resource = load('catalog-resource')
        self.feedback = load('feedback')
    def cards(self, resources=None, feedback=None, context=None, now=NOW):
        context = self.context if context is None else context
        resources = [self.resource] if resources is None else resources
        feedback = [] if feedback is None else feedback
        validator('project-context').validate(context)
        for r in resources: validator('catalog-resource').validate(r)
        for f in feedback: validator('feedback').validate(f)
        result = policy.recommend(context,resources,feedback,now)
        validator('recommendation-packet').validate(result)
        return result['useful_discoveries']

    def test_six_schemas_and_synthetic_examples_validate(self):
        paths = sorted((SPEC/'contracts').glob('*.schema.json'))
        self.assertEqual(len(paths),6)
        for path in paths:
            name = path.name.removesuffix('.schema.json')
            Draft202012Validator.check_schema(json.loads(path.read_text()))
            validator(name).validate(load(name))

    def test_output_contract_forbids_more_than_three(self):
        packet=load('recommendation-packet'); packet['useful_discoveries']*=4
        with self.assertRaises(ValidationError): validator('recommendation-packet').validate(packet)

    def test_output_contract_forbids_authorized_action(self):
        packet=load('recommendation-packet'); packet['useful_discoveries'][0]['authorized_actions']=['install']
        with self.assertRaises(ValidationError): validator('recommendation-packet').validate(packet)

    def test_import_contract_rejects_authority_and_execution_fields(self):
        for key,value in [('reviewed',True),('approval','yes'),('adapter_id','x'),('command','echo hi'),('secret','canary'),('project_id','private')]:
            with self.subTest(key=key):
                payload=load('candidate-import');payload['candidates'][0][key]=value
                with self.assertRaises(ValidationError): validator('candidate-import').validate(payload)

    def test_import_rejects_overlong_and_too_many_candidates(self):
        payload=load('candidate-import');payload['candidates']*=51
        with self.assertRaises(ValidationError): validator('candidate-import').validate(payload)
        payload=load('candidate-import');payload['candidates'][0]['description']='x'*601
        with self.assertRaises(ValidationError): validator('candidate-import').validate(payload)

    def test_url_rejects_local_execution_and_embedded_credentials(self):
        for url in ('file:///C:/secrets','javascript:alert(1)','http://example.org/r','https://token@example.org/r','https://example.org/r?token=canary'):
            with self.subTest(url=url):
                payload=load('candidate-import');payload['candidates'][0]['canonical_url']=url
                with self.assertRaises(ValidationError):validator('candidate-import').validate(payload)
                with self.assertRaises(ValueError):policy.canonical_source(url)

    def test_reference_only_cannot_smuggle_adapter(self):
        self.resource['adoption']['adapter_id']='example-installer'
        with self.assertRaises(ValidationError):validator('catalog-resource').validate(self.resource)

    def test_review_status_requires_revision_and_time(self):
        self.resource['review']['reviewed_revision']=None
        with self.assertRaises(ValidationError):validator('catalog-resource').validate(self.resource)

    def test_research_brief_excludes_private_extra_fields(self):
        for key in ('project_id','repository','path','endpoint','transcript','api_key','command'):
            with self.subTest(key=key):
                brief=load('research-brief');brief[key]='PRIVATE_CANARY'
                with self.assertRaises(ValidationError):validator('research-brief').validate(brief)
        text=json.dumps(load('research-brief'))
        self.assertNotIn(self.context['project_id'],text)
        self.assertNotIn(self.context['evidence_refs'][0],text)

    def test_feedback_requires_snooze_time(self):
        self.feedback['disposition']='snoozed'
        with self.assertRaises(ValidationError):validator('feedback').validate(self.feedback)

    def test_observed_benefit_requires_local_evaluation_evidence(self):
        self.feedback['disposition']='evaluated';self.feedback['outcome']='useful'
        with self.assertRaises(ValidationError):validator('feedback').validate(self.feedback)

    def test_relevant_explicit_need_is_explained(self):
        card=self.cards()[0]
        self.assertEqual(card['matched_need'],'interface_review')
        self.assertEqual(card['evidence_refs'],['approved-task-example'])
        self.assertEqual(card['authorized_actions'],[])
        self.assertEqual(card['benefit_status'],'potential')

    def test_unrelated_project_gets_no_recommendation(self):
        self.context['needs']=['credential_location']
        self.assertEqual(self.cards(),[])

    def test_no_approved_focus_no_invented_need(self):
        self.context['focus_approved']=False
        self.assertEqual(self.cards(),[])
        self.context['focus_approved']=True;self.context['needs']=[]
        self.assertEqual(self.cards(),[])

    def test_no_supporting_evidence_stays_quiet(self):
        self.context['evidence_refs']=[]
        self.assertEqual(self.cards(),[])

    def test_empty_catalog_stays_empty(self):
        self.assertEqual(self.cards(resources=[]),[])

    def test_existing_applicable_equivalent_is_preferred(self):
        existing=fresh_resource('z-existing');existing['equivalence_key']=self.resource['equivalence_key']
        self.context['inventory']=[{'resource_id':'z-existing','state':'available','coverage':'complete'}]
        cards=self.cards([self.resource,existing])
        self.assertEqual([c['resource_id'] for c in cards],['z-existing'])
        self.assertEqual(cards[0]['next_step'],'use_existing_reference')

    def test_shared_need_alone_is_not_equivalence(self):
        other=fresh_resource('other-resource')
        self.assertEqual(len(self.cards([self.resource,other])),2)

    def test_partial_or_denied_inventory_is_unknown(self):
        for cov in ['partial','denied','unsupported','timed_out']:
            with self.subTest(coverage=cov):
                self.context['inventory']=[{'resource_id':self.resource['id'],'state':'absent','coverage':cov}]
                self.assertEqual(self.cards()[0]['availability'],'unknown')

    def test_limit_is_three_without_padding(self):
        many=[fresh_resource(f'example-{i}') for i in range(8)]
        self.assertEqual(len(self.cards(many)),3)
        self.assertEqual(len(self.cards(many[:1])),1)

    def test_result_is_deterministic_not_catalog_order(self):
        many=[fresh_resource(f'example-{i}') for i in range(5)]
        self.assertEqual(self.cards(many),self.cards(list(reversed(many))))

    def test_paid_resource_is_excluded_under_free_constraint(self):
        self.resource['compatibility']['cost']='paid'
        self.assertEqual(self.cards(),[])

    def test_unknown_cost_is_needs_review_not_ready(self):
        self.resource['compatibility']['cost']='unknown'
        self.assertEqual(self.cards()[0]['compatibility_status'],'needs_review')

    def test_unknown_licensing_is_scoped_and_disclosed(self):
        self.resource['license']['status']='mixed'
        card=self.cards()[0]
        self.assertEqual(card['license']['status'],'mixed')
        self.assertEqual(card['compatibility_status'],'needs_review')

    def test_incompatible_platform_is_excluded(self):
        self.resource['compatibility']['platforms']=['macos']
        self.assertEqual(self.cards(),[])

    def test_unverified_version_requirements_need_review(self):
        self.resource['compatibility']['version_requirements']=['example version 3 only']
        self.assertEqual(self.cards()[0]['next_step'],'review_applicability')

    def test_privacy_and_network_constraints_apply(self):
        self.resource['compatibility']['privacy']='external'
        self.assertEqual(self.cards(),[])
        self.resource['compatibility']['privacy']='local';self.resource['compatibility']['requires_network']=True
        self.assertEqual(self.cards(),[])

    def test_accepted_decision_blocks_candidate_and_alias(self):
        other=fresh_resource('alias');other['canonical_url']=self.resource['canonical_url']+'#section'
        self.context['constraints']['blocked_resource_ids']=[self.resource['id']]
        self.assertEqual(self.cards([self.resource,other]),[])

    def test_dismissal_is_project_and_need_scoped(self):
        self.assertEqual(self.cards(feedback=[self.feedback]),[])
        self.context['project_id']='another-project'
        self.assertEqual(len(self.cards(feedback=[self.feedback])),1)
        self.context['project_id']='example-project';self.context['needs']=['accessibility_review']
        self.assertEqual(len(self.cards(feedback=[self.feedback])),1)

    def test_dismissal_persists_through_known_alias_and_revision(self):
        alias=fresh_resource('alias');alias['canonical_url']=self.resource['canonical_url']+'#section'
        alias['review']['reviewed_revision']='fixture-r2'
        self.assertEqual(self.cards([self.resource,alias],feedback=[self.feedback]),[])

    def test_snooze_expires_without_spreading_to_other_project(self):
        self.feedback.update(disposition='snoozed',until='2026-09-19T00:00:00Z')
        self.assertEqual(self.cards(feedback=[self.feedback]),[])
        self.assertEqual(len(self.cards(feedback=[self.feedback],now='2026-09-20T00:00:00Z')),1)

    def test_future_feedback_does_not_apply_early(self):
        self.feedback['recorded_at']='2026-09-19T00:00:00Z'
        self.assertEqual(len(self.cards(feedback=[self.feedback])),1)

    def test_latest_explicit_feedback_wins(self):
        later=copy.deepcopy(self.feedback);later.update(feedback_id='later',recorded_at='2026-09-18T01:00:00Z',disposition='saved')
        self.assertEqual(len(self.cards(feedback=[later,self.feedback])),1)

    def test_stale_catalog_has_explicit_review_warning(self):
        self.resource['review']['reviewed_at']='2026-01-01T00:00:00Z'
        card=self.cards()[0]
        self.assertEqual(card['review_freshness'],'stale')
        self.assertEqual(card['next_step'],'review_applicability')

    def test_future_review_cannot_create_current_trust(self):
        self.resource['review']['reviewed_at']='2026-09-19T00:00:00Z'
        self.assertEqual(self.cards(),[])

    def test_unverified_lead_not_promoted_into_normal_context(self):
        self.resource['review'].update(status='unverified',reviewed_revision=None,reviewed_at=None)
        self.assertEqual(self.cards(),[])

    def test_saved_or_adopted_is_not_demonstrated_benefit(self):
        for value in ['saved','adopted']:
            self.feedback['disposition']=value
            self.assertEqual(self.cards(feedback=[self.feedback])[0]['benefit_status'],'potential')

    def test_evaluation_is_revision_specific(self):
        self.feedback.update(disposition='evaluated',outcome='useful',evidence_ref='trial-evidence')
        self.assertEqual(self.cards(feedback=[self.feedback])[0]['benefit_status'],'observed_useful_here')
        self.resource['review']['reviewed_revision']='fixture-r2'
        self.assertEqual(self.cards(feedback=[self.feedback])[0]['benefit_status'],'potential')

    def test_duplicate_source_is_not_repeated(self):
        alias=fresh_resource('alias');alias['canonical_url']=self.resource['canonical_url']+'#more'
        self.assertEqual(len(self.cards([self.resource,alias])),1)

    def test_duplicate_id_and_conflicting_inventory_fail_closed(self):
        with self.assertRaises(ValueError):self.cards([self.resource,copy.deepcopy(self.resource)])
        item={'resource_id':self.resource['id'],'state':'available','coverage':'complete'}
        self.context['inventory']=[item,copy.deepcopy(item)]
        with self.assertRaises(ValueError):self.cards()

    def test_all_resource_kinds_work_as_reference_only(self):
        for kind in ['existing_capability','tool','integration','skill','workflow','documentation','reference_implementation','dataset','benchmark']:
            with self.subTest(kind=kind):
                self.resource['kind']=kind
                card=self.cards()[0]
                self.assertEqual(card['adoption_mode'],'reference_only')
                self.assertEqual(card['authorized_actions'],[])

    def test_all_28_product_cases_remain_not_run(self):
        cases=json.loads((SPEC/'acceptance.json').read_text())['cases']
        self.assertEqual(len(cases),28)
        self.assertEqual(len({x['id'] for x in cases}),28)
        self.assertTrue(all(c['product_test_status']=='not_run_product' for c in cases))

    def test_r0_native_build_and_schema_are_byte_identical(self):
        manifest=json.loads((ROOT/'validation/r0-baseline-preservation.json').read_text())
        self.assertGreater(len(manifest['preserved_files']),30)
        for entry in manifest['preserved_files']:
            path=ROOT/entry['path']
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),entry['sha256'],entry['path'])

    def test_source_status_does_not_claim_live_discovery(self):
        scope=json.loads((ROOT/'scope.json').read_text())
        self.assertEqual(scope['module_count'],5)
        self.assertFalse(scope['r0_runtime_change'])
        addon=scope['accepted_addenda'][0]
        self.assertEqual(addon['implementation_stage'],'R4')
        self.assertEqual(addon['current_runtime_status'],'not_implemented')

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path,default=ROOT/'validation/discovery-spec-checks.json')
    args=parser.parse_args()
    start=time.monotonic();suite=unittest.defaultTestLoader.loadTestsFromTestCase(DiscoverySpecificationTests)
    ids=[test.id() for test in suite]
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    report={'scope':'FS-1.0-D1','kind':'development_only_schema_reference_policy_and_preservation_checks',
            'tests_run':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),
            'elapsed_seconds':round(time.monotonic()-start,4),'test_ids':ids,
            'rust_application_executed':False,'windows_executed':False,'live_resource_catalog_reviewed':False,
            'discovery_product_implemented':False,'product_acceptance_cases':28,'product_acceptance_cases_executed':0,
            'failure_details':[detail for _,detail in result.failures+result.errors],
            'limitations':['The pure Python reference tests do not run a Rust matcher or Workstation context command.',
                           'Input shape tests do not prove import/export privacy, rendering safety, MCP boundaries or approval enforcement.',
                           'All catalog examples are synthetic; no real external resource is reviewed or recommended here.']}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    raise SystemExit(0 if result.wasSuccessful() else 1)
