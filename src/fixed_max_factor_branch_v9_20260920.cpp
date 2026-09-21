// Runtime-configured, self-reporting version of v8; propagation is unchanged.
// Hybrid sparse/bit-parallel exact factor branching with two watched factor
// options and an undo trail.  Any cap is UNKNOWN.  NO emits the same complete
// prefix proof as earlier versions.
#include <algorithm>
#include <chrono>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <tuple>
#include <unordered_map>
#include <utility>
#include <vector>

// The branch window is an explicit runtime parameter, never inferred from a filename.

using Clock=std::chrono::steady_clock;
enum class Result { NO,YES,UNKNOWN };
struct Proof {
  bool conflict=false;int a=0,b=0,sum=0;
  std::vector<std::pair<int,int>> options;
  std::vector<std::unique_ptr<Proof>> children;
};
struct State {
  struct WordUndo {int index;uint64_t old;};
  struct WatchUndo {int sum;uint16_t first,second;};
  std::vector<uint8_t> member;
  std::vector<uint64_t> member_bits,banned_bits,active_bits,done_bits;
  std::vector<uint16_t> watch1,watch2;
  std::vector<int> members,demands,reason_a,reason_b;
  std::vector<WordUndo> banned_undo;
  std::vector<int> active_undo,done_undo;
  std::vector<WatchUndo> watch_undo;
  size_t processed=0;
};
struct Checkpoint {
  size_t members,demands,processed,banned,active,done,watches;
};
struct Solver {
  int n,branch_lookahead;uint64_t node_cap,nodes=0,branches=0,conflicts=0,translate_words=0,
      new_demands=0,demand_scans=0,watch_rescans=0,factor_visits=0,
      prime_words=0,prime_ban_bits=0,sparse_pair_checks=0;
  double second_cap;Clock::time_point started=Clock::now();bool capped=false;
  std::vector<uint8_t> prime;std::vector<int> spf,example;
  std::vector<uint64_t> prime_bits;
  std::unordered_map<int,std::vector<std::pair<int,int>>> cache;
  Solver(int maximum,uint64_t maximum_nodes,double seconds,int lookahead):n(maximum),branch_lookahead(lookahead),node_cap(maximum_nodes),
    second_cap(seconds),prime(2*n+1,1),spf(2*n+1,0),prime_bits((2*n+64)/64,0) {
    prime[0]=prime[1]=0;
    for(int p=2;p<=2*n;++p)if(!spf[p]){
      spf[p]=p;
      if(int64_t(p)*p<=2*n)for(int q=p*p;q<=2*n;q+=p)if(!spf[q])spf[q]=p;
    }
    for(int v=2;v<=2*n;++v){prime[v]=(spf[v]==v);if(prime[v])prime_bits[v>>6]|=uint64_t(1)<<(v&63);}
  }
  double elapsed()const{return std::chrono::duration<double>(Clock::now()-started).count();}
  bool budget(){if(nodes>node_cap||elapsed()>second_cap)capped=true;return capped;}
  static bool bit(const std::vector<uint64_t>&bits,int v){return bits[v>>6]>>(v&63)&1;}
  static void setbit(std::vector<uint64_t>&bits,int v){bits[v>>6]|=uint64_t(1)<<(v&63);}
  // Bit k of the result is source[start+k].
  static uint64_t window(const std::vector<uint64_t>&source,int start){
    if(start<=-64)return 0;
    if(start<0)return source.empty()?0:source[0]<<(-start);
    size_t index=size_t(start)>>6;unsigned offset=unsigned(start)&63;
    if(index>=source.size())return 0;
    uint64_t result=source[index]>>offset;
    if(offset&&index+1<source.size())result|=source[index+1]<<(64-offset);
    return result;
  }
  const std::vector<std::pair<int,int>>& factors(int s){
    auto it=cache.find(s);if(it!=cache.end())return it->second;
    int rest=s;std::vector<int> divisors{1};
    while(rest>1){int p=spf[rest],power=1;size_t old=divisors.size();do{
      rest/=p;power*=p;for(size_t i=0;i<old;++i)divisors.push_back(divisors[i]*power);
    }while(rest%p==0);}
    auto&out=cache[s];for(int d:divisors)if(int64_t(d)*d<=s&&s/d<=n)out.emplace_back(d,s/d);
    std::sort(out.begin(),out.end());return out;
  }
  bool banned(const State&st,int v)const{return bit(st.banned_bits,v);}
  bool live(const State&st,const std::pair<int,int>&option)const{
    return !banned(st,option.first)&&!banned(st,option.second);
  }
  bool satisfied(const State&st,const std::pair<int,int>&option)const{
    return st.member[option.first]&&st.member[option.second];
  }
  bool add(State&st,int v){
    if(v<1||v>n||banned(st,v))return false;if(st.member[v])return true;
    st.member[v]=1;setbit(st.member_bits,v);st.members.push_back(v);return true;
  }
  bool ban(State&st,int v){
    if(v<1||v>n)return true;if(st.member[v])return false;
    int word=v>>6;uint64_t mask=uint64_t(1)<<(v&63);
    if(!(st.banned_bits[word]&mask)){st.banned_undo.push_back({word,st.banned_bits[word]});st.banned_bits[word]|=mask;}
    return true;
  }
  void mark_active(State&st,int s){
    if(!bit(st.active_bits,s)){setbit(st.active_bits,s);st.active_undo.push_back(s);}
  }
  void mark_done(State&st,int s){
    if(!bit(st.done_bits,s)){setbit(st.done_bits,s);st.done_undo.push_back(s);}
  }
  void set_watch(State&st,int s,uint16_t first,uint16_t second){
    st.watch_undo.push_back({s,st.watch1[s],st.watch2[s]});st.watch1[s]=first;st.watch2[s]=second;
  }
  Checkpoint checkpoint(const State&st)const{
    return {st.members.size(),st.demands.size(),st.processed,st.banned_undo.size(),
      st.active_undo.size(),st.done_undo.size(),st.watch_undo.size()};
  }
  void rollback(State&st,const Checkpoint&point){
    while(st.watch_undo.size()>point.watches){auto u=st.watch_undo.back();st.watch_undo.pop_back();st.watch1[u.sum]=u.first;st.watch2[u.sum]=u.second;}
    while(st.done_undo.size()>point.done){int s=st.done_undo.back();st.done_undo.pop_back();st.done_bits[s>>6]&=~(uint64_t(1)<<(s&63));}
    while(st.active_undo.size()>point.active){int s=st.active_undo.back();st.active_undo.pop_back();st.active_bits[s>>6]&=~(uint64_t(1)<<(s&63));}
    while(st.banned_undo.size()>point.banned){auto u=st.banned_undo.back();st.banned_undo.pop_back();st.banned_bits[u.index]=u.old;}
    while(st.members.size()>point.members){int v=st.members.back();st.members.pop_back();st.member[v]=0;st.member_bits[v>>6]&=~(uint64_t(1)<<(v&63));}
    st.demands.resize(point.demands);st.processed=point.processed;
  }
  bool apply_prime_bans(State&st,int a){
    const int low=n+1-a,first_word=std::max(1,low)>>6,last_word=n>>6;
    for(int w=first_word;w<=last_word;++w){
      ++prime_words;int base=w<<6;uint64_t mask=window(prime_bits,base+a);
      if(base<low)mask&=~uint64_t(0)<<(low-base);
      if(base+63>n)mask&=~uint64_t(0)>>(base+63-n);
      if(st.member_bits[w]&mask){++conflicts;return false;}
      prime_ban_bits+=__builtin_popcountll(mask);uint64_t updated=st.banned_bits[w]|mask;
      if(updated!=st.banned_bits[w]){st.banned_undo.push_back({w,st.banned_bits[w]});st.banned_bits[w]=updated;}
    }
    return true;
  }
  bool activate_sum(State&st,int a,int s){
    if(prime[s]){if(s>n||!add(st,s)){++conflicts;return false;}return true;}
    if(bit(st.active_bits,s))return true;
    const auto&options=factors(s);int first=-1,second=-1;bool covered=false;
    for(size_t i=0;i<options.size();++i){++factor_visits;
      if(satisfied(st,options[i])){covered=true;break;}
      if(live(st,options[i])){if(first<0)first=int(i);else if(second<0)second=int(i);}
    }
    mark_active(st,s);
    if(covered){mark_done(st,s);return true;}
    if(first<0){++conflicts;return false;}
    if(second<0){auto[d,e]=options[first];if(!add(st,d)||!add(st,e)){++conflicts;return false;}
      mark_done(st,s);return true;}
    ++new_demands;st.demands.push_back(s);st.reason_a[s]=a;st.reason_b[s]=s-a;
    set_watch(st,s,uint16_t(first),uint16_t(second));return true;
  }
  bool activate_translates(State&st,int a){
    const size_t snapshot=st.members.size(),word_count=size_t(n+63)/64;
    if(snapshot<word_count){
      for(size_t i=0;i<snapshot;++i){if((++sparse_pair_checks&8191)==0&&budget())return false;
        if(!activate_sum(st,a,a+st.members[i]))return false;}
      return true;
    }
    const int first_word=(a+1)>>6,last_word=(a+n)>>6;
    for(int w=first_word;w<=last_word;++w){
      if((++translate_words&8191)==0&&budget())return false;
      int base=w<<6;uint64_t sums=window(st.member_bits,base-a);
      if(base<2)sums&=~uint64_t(0)<<(2-base);
      if(base+63>2*n)sums&=~uint64_t(0)>>(base+63-2*n);
      uint64_t forced=sums&prime_bits[w];
      if(base<=n){
        uint64_t low_mask=(base+63<=n)?~uint64_t(0):(~uint64_t(0)>>(base+63-n));
        forced&=low_mask;
        while(forced){int k=__builtin_ctzll(forced),s=base+k;forced&=forced-1;if(!add(st,s)){++conflicts;return false;}}
      }else if(forced){++conflicts;return false;}
      uint64_t candidates=sums&~prime_bits[w]&~st.active_bits[w];
      while(candidates){
        int k=__builtin_ctzll(candidates),s=base+k;candidates&=candidates-1;
        if(!activate_sum(st,a,s))return false;
      }
    }
    return true;
  }
  // -1 conflict, 0 fixed point, 1 cap.
  int propagate(State&st){
    for(;;){
      while(st.processed<st.members.size()){
        if(budget())return 1;int a=st.members[st.processed++];
        if(!apply_prime_bans(st,a))return -1;
        if(!activate_translates(st,a))return capped?1:-1;
      }
      bool changed=false;
      for(int s:st.demands){
        if((++demand_scans&8191)==0&&budget())return 1;
        if(bit(st.done_bits,s))continue;
        const auto&options=factors(s);auto one=options[st.watch1[s]],two=options[st.watch2[s]];
        if(live(st,one)&&live(st,two))continue;
        ++watch_rescans;bool covered=false;int count=0,first=-1,second=-1;
        for(size_t i=0;i<options.size();++i){++factor_visits;
          if(satisfied(st,options[i])){covered=true;break;}
          if(live(st,options[i])){if(count==0)first=int(i);else if(count==1)second=int(i);++count;}
        }
        if(covered){mark_done(st,s);continue;}
        if(count==0){++conflicts;return -1;}
        if(count==1){size_t old=st.members.size();auto[d,e]=options[first];
          if(!add(st,d)||!add(st,e)){++conflicts;return -1;}
          mark_done(st,s);
          changed|=st.members.size()>old;
        }else{
          set_watch(st,s,uint16_t(first),uint16_t(second));
        }
      }
      if(!changed&&st.processed==st.members.size())return 0;
    }
  }
  bool choose(State&st,int&a,int&b,int&s,std::vector<std::pair<int,int>>&best){
    s=0;best.clear();int fallback=0,binary_seen=0,best_endpoints=0;
    for(int total:st.demands){
      if(bit(st.done_bits,total))continue;
      bool covered=false;std::vector<std::pair<int,int>>options_live;
      for(auto option:factors(total)){
        ++factor_visits;auto[d,e]=option;
        if(st.member[d]&&st.member[e]){covered=true;break;}
        if(!banned(st,d)&&!banned(st,e)&&options_live.size()<3)options_live.push_back(option);
      }
      if(covered){mark_done(st,total);continue;}
      if(options_live.size()<2)throw std::runtime_error("missed unit demand");
      if(options_live.size()==2){
        int endpoints=0;for(auto[d,e]:options_live)endpoints+=(!st.member[d])+(d!=e&&!st.member[e]);
        if(!s||std::make_tuple(endpoints,total<=n,-total)<std::make_tuple(best_endpoints,s<=n,-s)){
          s=total;a=st.reason_a[total];b=st.reason_b[total];best=std::move(options_live);best_endpoints=endpoints;
        }
        if(++binary_seen>=branch_lookahead)return true;
        continue;
      }
      if(!fallback)fallback=total;
    }
    if(s)return true;
    if(!fallback)return false;
    s=fallback;a=st.reason_a[s];b=st.reason_b[s];best.clear();
    for(auto option:factors(s))if(live(st,option))best.push_back(option);
    return true;
  }
  Result dfs(State&st,std::unique_ptr<Proof>&proof){
    ++nodes;if(budget())return Result::UNKNOWN;int p=propagate(st);
    if(p>0)return Result::UNKNOWN;if(p<0){proof=std::make_unique<Proof>();proof->conflict=true;return Result::NO;}
    int a=0,b=0,s=0;std::vector<std::pair<int,int>>options;
    if(!choose(st,a,b,s,options)){std::sort(st.members.begin(),st.members.end());example=st.members;return Result::YES;}
    auto node=std::make_unique<Proof>();node->a=a;node->b=b;node->sum=s;node->options=options;
    for(auto[d,e]:options){
      if(budget())return Result::UNKNOWN;++branches;Checkpoint point=checkpoint(st);
      if(!add(st,d)||!add(st,e)){auto leaf=std::make_unique<Proof>();leaf->conflict=true;node->children.push_back(std::move(leaf));rollback(st,point);continue;}
      std::unique_ptr<Proof>child_proof;Result result=dfs(st,child_proof);rollback(st,point);
      if(result!=Result::NO)return result;node->children.push_back(std::move(child_proof));
    }
    proof=std::move(node);return Result::NO;
  }
  void write(std::ostream&out,const Proof&p)const{
    if(p.conflict){out<<"C\n";return;}out<<"B "<<p.a<<' '<<p.b<<' '<<p.sum<<' '<<p.options.size();
    for(auto[d,e]:p.options)out<<' '<<d<<' '<<e;out<<'\n';for(auto&child:p.children)write(out,*child);
  }
};
int main(int argc,char**argv){try{
  if(argc!=8)throw std::runtime_error("Usage: solver N state seconds nodes proof result lookahead");
  int n=std::stoi(argv[1]);double seconds=std::stod(argv[3]);uint64_t nodes=std::stoull(argv[4]);
  int lookahead=std::stoi(argv[7]);
  if(n<2||n>1000000||!(seconds>0)||!nodes||lookahead<1||lookahead>4096)throw std::runtime_error("Invalid configuration");
  Solver solver(n,nodes,seconds,lookahead);State root;root.member.assign(n+1,0);
  root.member_bits.assign((n+64)/64,0);root.banned_bits.assign((n+64)/64,0);
  root.active_bits.assign((2*n+64)/64,0);root.done_bits.assign((2*n+64)/64,0);
  root.watch1.assign(2*n+1,0);root.watch2.assign(2*n+1,0);
  root.reason_a.assign(2*n+1,0);root.reason_b.assign(2*n+1,0);
  std::ifstream input(argv[2]);if(!input)throw std::runtime_error("State input error");char tag;int value;
  while(input>>tag>>value){if(value<1||value>n)throw std::runtime_error("Bad state value");
    if(tag=='F'){if(!solver.add(root,value))throw std::runtime_error("Contradictory root");}
    else if(tag=='B'){if(!solver.ban(root,value))throw std::runtime_error("Contradictory root");}
    else throw std::runtime_error("Bad state tag");}
  if(!root.member[1]||!root.member[2]||!root.member[n])throw std::runtime_error("Missing seeds");
  root.banned_undo.clear();root.active_undo.clear();root.done_undo.clear();root.watch_undo.clear();
  std::unique_ptr<Proof>proof;Result result=solver.dfs(root,proof);const char*status=result==Result::NO?"NO":result==Result::YES?"YES":"UNKNOWN";
  if(result==Result::NO){std::ofstream out(argv[5]);solver.write(out,*proof);out.flush();if(!out)throw std::runtime_error("Proof output error");}
  else if(result==Result::YES){std::ofstream out(argv[5]);out<<"Y "<<solver.example.size();for(int v:solver.example)out<<' '<<v;out<<'\n';}
  std::ofstream out(argv[6]);out<<"{\"maximum\":"<<n<<",\"status\":\""<<status<<"\",\"seconds\":"<<solver.elapsed()
    <<",\"algorithm\":\"factor_branch_runtime_v9\",\"branch_lookahead\":"<<lookahead<<",\"seconds_cap\":"<<seconds<<",\"node_cap\":"<<nodes
    <<",\"nodes\":"<<solver.nodes<<",\"branches\":"<<solver.branches<<",\"conflicts\":"<<solver.conflicts
    <<",\"translate_words\":"<<solver.translate_words<<",\"new_demands\":"<<solver.new_demands
    <<",\"demand_scans\":"<<solver.demand_scans<<",\"watch_rescans\":"<<solver.watch_rescans
    <<",\"factor_visits\":"<<solver.factor_visits
    <<",\"prime_words\":"<<solver.prime_words<<",\"prime_ban_bits\":"<<solver.prime_ban_bits
    <<",\"sparse_pair_checks\":"<<solver.sparse_pair_checks
    <<",\"factor_cache_sums\":"<<solver.cache.size()<<",\"example_size\":"<<solver.example.size()<<"}\n";
  std::cout<<status<<" nodes="<<solver.nodes<<" seconds="<<solver.elapsed()<<'\n';
}catch(const std::exception&e){std::cerr<<e.what()<<'\n';return 1;}}
