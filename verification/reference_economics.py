"""Independent executable specification, used for vectors only. NOT product/runtime code."""
import math

def forecast(samples,now):
 if not samples or len(samples)>10000:raise ValueError('SAMPLE_COUNT')
 for s in samples:
  if not math.isfinite(s['used']) or s['used']<0 or s['used']>1e15:raise ValueError('INVALID_USAGE')
  if s['limit'] is not None and (not math.isfinite(s['limit']) or s['limit']<=0):raise ValueError('INVALID_QUOTA')
  allowed={'subscription':['percent','tokens','requests','credits'],'context':['tokens','percent'],'dollars':['usd']}
  if s['unit'] not in allowed[s['meter']]:raise ValueError('INCOMPATIBLE_METER_UNIT')
 rows=sorted((s for s in samples if s['observed_at']<=now),key=lambda s:s['observed_at'])
 if not rows:raise ValueError('NO_NONFUTURE_SAMPLE')
 last=rows[-1]
 if any(any(s[k]!=last[k] for k in ('provider','account_alias','bucket','meter','unit')) for s in rows):raise ValueError('MIXED_STREAMS')
 remaining=None if last['limit'] is None else max(0,last['limit']-last['used'])
 result={'remaining':remaining,'confidence':'insufficient','horizons':[]}
 if last['meter']!='subscription' or (last['reset_at'] is not None and last['reset_at']<=now) or now-last['observed_at']>900 or remaining is None:return result
 if len({s['observed_at'] for s in rows})!=len(rows):raise ValueError('DUPLICATE_SAMPLE_TIME')
 segment=[last]
 for s in reversed(rows[:-1]):
  if any(s[k]!=last[k] for k in ('window_id','reset_at','limit')) or s['used']>segment[-1]['used'] or segment[-1]['observed_at']-s['observed_at']>21600:break
  segment.append(s)
 segment.reverse();span=last['observed_at']-segment[0]['observed_at']
 if len(segment)<2 or span<60:return result
 reset=None if last['reset_at'] is None else last['reset_at']-now
 def h(label,rate,elapsed):
  eta=0. if remaining<=0 else (remaining/rate if rate is not None and rate>0 else None)
  return {'label':label,'observed_span_seconds':elapsed,'rate_per_second':rate,'capacity_at_current_pace_seconds':eta,'exhaustion_after_seconds':None if eta is not None and reset is not None and eta>=reset else eta}
 for label,width in [('10m',600),('1h',3600),('6h',21600)]:
  elapsed=0;burn=0.
  for a,b in zip(segment,segment[1:]):
   overlap=max(0,b['observed_at']-max(a['observed_at'],last['observed_at']-width));dt=b['observed_at']-a['observed_at'];burn+=(b['used']-a['used'])*overlap/dt;elapsed+=overlap
  result['horizons'].append(h(label,burn/elapsed if elapsed>=60 else None,elapsed))
 baseline=(last['used']-segment[0]['used'])/span;weighted=weights=0.
 for a,b in zip(segment,segment[1:]):
  dt=b['observed_at']-a['observed_at'];weight=dt*2**(-(last['observed_at']-b['observed_at'])/3600);weights+=weight;weighted+=(b['used']-a['used'])/dt*weight
 ewma=weighted/weights
 result['horizons'] += [h('observed_window_baseline',baseline,span),h('ewma_1h_half_life',ewma,span),h('blended',.65*ewma+.35*baseline,span)]
 result['confidence']='moderate' if len(segment)>=6 and span>=3600 else 'low'
 return result

def pareto(models):
 if any(m['license_state']!='permitted_local_use' for m in models):raise ValueError('BENCHMARK_LICENSE_NOT_PERMITTED')
 rows=[m for m in models if m['available']]
 if len({(m['benchmark'],m['revision']) for m in rows})>1:raise ValueError('INCOMPARABLE_BENCHMARKS')
 return [a['model'] for a in rows if not any(b['quality']>=a['quality'] and b['task_cost_usd']<=a['task_cost_usd'] and (b['quality']>a['quality'] or b['task_cost_usd']<a['task_cost_usd']) for b in rows)]
