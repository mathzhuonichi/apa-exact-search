// Fixed-maximum necessary propagation with explicit arithmetic explanations.
// A conclusion of FIXPOINT is not an existence result.
#include <algorithm>
#include <chrono>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <map>
#include <stdexcept>
#include <vector>

int main(int argc, char** argv) {
  try {
    if (argc != 4 && argc != 5) throw std::runtime_error("Usage: propagate N trace.txt state.txt [initial_state.txt]");
    const int n=std::stoi(argv[1]);
    if (n<113 || n>1000000) throw std::runtime_error("Maximum outside [113,1000000]");
    std::ofstream trace(argv[2]), state(argv[3]);
    if (!trace || !state) throw std::runtime_error("Output error");
    auto start=std::chrono::steady_clock::now();
    std::vector<int> spf(2*n+1,0);
    for (int p=2;p<=2*n;++p) if (!spf[p]) {
      spf[p]=p;
      if (int64_t(p)*p<=2*n) for (int q=p*p;q<=2*n;q+=p) if (!spf[q]) spf[q]=p;
    }
    auto prime=[&](int v) { return v>=2 && spf[v]==v; };
    std::vector<uint8_t> allowed(n+1,1), mandatory(n+1,0);
    std::vector<int> forced{1,2,3,5,7,11,13,17,19,23,31,43,47,53,61,71,73,83,103,109,113};
    if (std::find(forced.begin(),forced.end(),n)==forced.end()) forced.push_back(n);
    int initial_bans=0;
    if(argc==5) {
      std::ifstream input(argv[4]);if(!input)throw std::runtime_error("Initial-state error");
      forced.clear();char tag;int v;
      while(input>>tag>>v) {
        if(v<1||v>n)throw std::runtime_error("Initial value outside universe");
        if(tag=='F')forced.push_back(v);
        else if(tag=='B'){allowed[v]=0;++initial_bans;}
        else throw std::runtime_error("Initial-state tag error");
      }
      std::sort(forced.begin(),forced.end());
      if(std::adjacent_find(forced.begin(),forced.end())!=forced.end())throw std::runtime_error("Duplicate initial member");
    }
    for(int v:forced) { mandatory[v]=1; if(argc==4)trace<<"S "<<v<<'\n'; }
    if(!mandatory[1]||!mandatory[2]||!mandatory[n])throw std::runtime_error("Missing initial mandatory member");
    std::map<int,std::vector<std::pair<int,int>>> factor_cache;
    auto factors=[&](int s)->const std::vector<std::pair<int,int>>& {
      auto it=factor_cache.find(s);
      if(it!=factor_cache.end()) return it->second;
      int t=s;
      std::vector<int> divisors{1};
      while(t>1) {
        int p=spf[t],power=1;
        size_t old=divisors.size();
        do { t/=p;power*=p;for(size_t k=0;k<old;++k) divisors.push_back(divisors[k]*power); } while(t%p==0);
      }
      auto &result=factor_cache[s];
      for(int d:divisors) if(int64_t(d)*d<=s && s/d<=n) result.emplace_back(d,s/d);
      std::sort(result.begin(),result.end());return result;
    };
    std::vector<uint32_t> seen(n+1,0);
    std::vector<int> parent(n+1),step(n+1),queue,path;
    std::vector<int> new_roots;
    for(int v:forced) if(v%2) new_roots.push_back(v);
    uint32_t stamp=0;
    int bans=initial_bans,round=0;
    uint64_t queries=0;
    bool conflict=false;
    while(!conflict) {
      ++round;
      // Each new odd mandatory root is tested with shifts 2 and candidate e.
      std::sort(new_roots.begin(),new_roots.end(),std::greater<int>());
      if(!new_roots.empty()) for(int e=2;e<=n;e+=2) if(allowed[e]) {
        ++stamp;queue=new_roots;
        for(int r:new_roots) { seen[r]=stamp;parent[r]=-1; }
        bool bad=false;
        for(size_t k=0;k<queue.size() && !bad;++k) {
          const int a=queue[k];
          for(int shift:{2,e}) {
            ++queries;int p=a+shift;
            if(!prime(p)) continue;
            if(p>n) {
              path.clear();path.push_back(shift);int root=a;
              while(parent[root]>=0) { path.push_back(step[root]);root=parent[root]; }
              std::reverse(path.begin(),path.end());
              trace<<"B "<<e<<' '<<root<<' '<<path.size();
              for(int v:path) trace<<' '<<v;
              trace<<'\n';allowed[e]=0;++bans;bad=true;
              if(mandatory[e]) { trace<<"C "<<e<<'\n';conflict=true; }
              break;
            }
            if(seen[p]!=stamp) { seen[p]=stamp;parent[p]=a;step[p]=shift;queue.push_back(p); }
          }
        }
        if(conflict) break;
      }
      new_roots.clear();
      if(conflict) break;
      bool changed=true;
      bool yield_to_unary=false;
      while(changed && !conflict && !yield_to_unary) {
        changed=false;
        std::sort(forced.begin(),forced.end(),std::greater<int>());
        const size_t old_size=forced.size();
        for(size_t i=0;i<old_size && !conflict && !yield_to_unary;++i)
          for(size_t j=i;j<old_size && !yield_to_unary;++j) {
          const int a=forced[i],b=forced[j],s=a+b;
          std::vector<std::pair<int,int>> live;
          for(auto pair:factors(s)) if(allowed[pair.first] && allowed[pair.second]) live.push_back(pair);
          if(live.empty()) { trace<<"X "<<a<<' '<<b<<'\n';conflict=true;break; }
          for(int v:{live[0].first,live[0].second}) {
            if(mandatory[v]) continue;
            bool common=true;
            for(auto pair:live) if(pair.first!=v && pair.second!=v) { common=false;break; }
            if(common) {
              mandatory[v]=1;forced.push_back(v);changed=true;
              if(v%2) new_roots.push_back(v);
              trace<<"F "<<v<<' '<<a<<' '<<b<<'\n';
              if(new_roots.size()>=64) { yield_to_unary=true;break; }
            }
          }
        }
      }
      std::cerr<<"round="<<round<<" forced="<<forced.size()<<" bans="<<bans<<" new_odd_roots="<<new_roots.size()<<'\n';
      if(new_roots.empty()) break;
    }
    std::sort(forced.begin(),forced.end());
    for(int v:forced) state<<"F "<<v<<'\n';
    for(int v=1;v<=n;++v) if(!allowed[v]) state<<"B "<<v<<'\n';
    trace.flush();state.flush();
    if(!trace || !state) throw std::runtime_error("Output failure");
    const double seconds=std::chrono::duration<double>(std::chrono::steady_clock::now()-start).count();
    std::cout<<"{\"maximum\":"<<n<<",\"status\":\""<<(conflict?"CONTRADICTION":"FIXPOINT")
      <<"\",\"forced_members\":"<<forced.size()<<",\"banned_members\":"<<bans
      <<",\"rounds\":"<<round<<",\"prime_queries\":"<<queries<<",\"seconds\":"<<seconds<<"}\n";
  } catch(const std::exception& e) { std::cerr<<e.what()<<'\n';return 1; }
}
