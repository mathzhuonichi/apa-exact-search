"""Independent replay of arithmetic disjunction evidence.
Uses trial division for bounded factor pairs, not the producer's SPF factoring.
Only the minimum-element lemma (1,2 in A) and the specified maximum are
initial premises. All other members are derived within this checker. Only Python standard-library modules are used.
"""
from pathlib import Path
from math import isqrt
from functools import lru_cache
import json,sys,time,gzip
N=221969;BASE=Path(__file__).resolve().parent
@lru_cache(None)
def prime(n):
 return n>=2 and all(n%d for d in range(2,isqrt(n)+1))
@lru_cache(None)
def factor_pairs(s):
 return tuple((d,s//d) for d in range(1,isqrt(s)+1) if s%d==0 and s//d<=N)
def live(s,B,G):
 return [p for p in factor_pairs(s) if not(set(p)&B) and p not in G]
def replay_reasons(F,B,G,reasons):
 F=set(F);B=set(B)
 for r in reasons:
  if r['kind']=='force':
   a,b=r['sum_pair'];v=r['value'];assert a in F and b in F,('unforced_sum',r)
   domain=live(a+b,B,G);assert domain and all(v in p for p in domain),('bad_force',r,domain)
   assert 1<=v<=N and v not in B
   F.add(v)
  elif r['kind']=='chain_ban':
   e=r['value'];known=set(F);assert 2 in F and 1<=e<=N
   for a,shift,v in r['steps']:
    assert a in known and a<=N and shift in (2,e) and a+shift==v and prime(v)
    known.add(v)
   assert r['steps'] and r['steps'][-1][2]>N
   B.add(e)
  elif r['kind']=='prime_ban':
   a=r['member'];v=r['value'];q=r['prime']
   assert a in F and q==a+v and q>N and prime(q) and 1<=v<=N
   B.add(v)
  elif r['kind']=='ban':
   a,b=r['nogood'];v=r['value'];assert (a,b) in G and v in (a,b)
   other=b if v==a else a;assert other in F
   B.add(v)
  else:raise AssertionError(r)
 return F,B
def check_end(F,B,G,end):
 kind=end['kind']
 if kind=='member_banned':assert end['value'] in F&B
 elif kind=='nogood':assert tuple(end['pair']) in G and set(end['pair'])<=F
 elif kind=='empty_domain':
  a,b=end['sum_pair'];assert a in F and b in F;assert not live(a+b,B,G),('nonempty_conflict',end,live(a+b,B,G))
 else:assert kind in ('FIXPOINT','LIMIT')
 return kind not in ('FIXPOINT','LIMIT')

def read_json(path):
    opener=gzip.open if path.suffix=='.gz' else open
    with opener(path,'rt',encoding='utf-8') as stream:return json.load(stream)

def verify_backbone():
    """Only 1, 2 and N are initial; the minimum-element lemma is in README."""
    F={1,2,N};B={38,74}
    assert all(prime(N+b) for b in B)
    def force(a,c):
        assert a in F and c in F
        domain=live(a+c,B,set());assert domain
        common=set(domain[0])
        for pair in domain[1:]:common.intersection_update(pair)
        F.update(common)
    for a,c in [(1,2),(2,3),(2,5),(N,2),(67,7),(37,1)]:force(a,c)
    assert F=={1,2,3,5,7,19,37,67,3313,N}
    return F

def main(path):
 start=time.monotonic();r=read_json(path);baseline=read_json(BASE/'root-premises.json.gz')
 # Independently check all initial unary prime-chain exclusions.
 baseF=set(baseline['forced']);B=set(map(int,baseline['ban_reasons']))
 for e,steps in baseline['ban_reasons'].items():
  e=int(e);known=set(baseF);last=None
  for a,shift,v in steps:
   assert a in known and shift in (2,e) and a+shift==v and prime(v)
   assert a<=N
   known.add(v);last=v
  assert last>N
 F=verify_backbone();assert F==set(r['base_members']) and baseF<=F;G=set();conflict=False;reason_count=0
 for idx,event in enumerate(r['events']):
  if event['kind']=='root_propagation':
   F,B=replay_reasons(F,B,G,event['reasons']);reason_count+=len(event['reasons']);assert not check_end(F,B,G,event['end'])
  elif event['kind']=='failed_absence':
   v=event['value'];assert 1<=v<=N
   f,b=replay_reasons(F,B|{v},G,event['reasons']);reason_count+=len(event['reasons'])
   assert check_end(f,b,G,event['end'])
   F.add(v)
  elif event['kind']=='root_conflict':
   F,B=replay_reasons(F,B,G,event['reasons']);reason_count+=len(event['reasons']);assert check_end(F,B,G,event['end']);conflict=True
  elif event['kind']=='split':
   a,b=event['sum_pair'];assert a in F and b in F
   options=[tuple(p) for p in event['options']];assert options==live(a+b,B,G),('incomplete_split',idx,event['sum_pair'])
   assert len(event['outcomes'])==len(options)
   survivors=[]
   for option,outcome in zip(options,event['outcomes']):
    assert tuple(outcome['assume_pair'])==option
    f,ban=replay_reasons(F|set(option),B,G,outcome['reasons']);reason_count+=len(outcome['reasons'])
    if check_end(f,ban,G,outcome['end']):G.add(option)
    else:survivors.append((f,ban))
   if event.get('conclusion')=='NO':assert not survivors;conflict=True
   else:
    assert survivors
    commonF=set.intersection(*(s[0] for s in survivors));commonB=set.intersection(*(s[1] for s in survivors))
    newF=set(event['new_members']);newB=set(event['new_bans'])
    assert newF<=commonF and newB<=commonB
    F.update(newF);B.update(newB)
  else:raise AssertionError(event['kind'])
 assert F==set(r['members']) and B==set(r['bans']) and G==set(map(tuple,r['nogoods']))
 assert (r['status']=='NO')==conflict
 out={'independent_arithmetic_replay':'PASS','result':r['status'],'events':len(r['events']),'arithmetic_steps':reason_count,'base_prime_chain_bans':len(baseline['ban_reasons']),'members':len(F),'seconds':time.monotonic()-start,'premise':'Only 1,2,N initially, with 1,2 justified by the minimum-element lemma. No historical seed list, maximum-deletion, or prior NO results are used. Not a formal proof assistant certificate.'}
 (BASE/'verification.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
if __name__=='__main__':
 if not __debug__:raise RuntimeError('Run this arithmetic checker without Python -O.')
 main(Path(sys.argv[1]) if len(sys.argv)>1 else BASE/'proof.json.gz')
