#include "../libraries/AogDevice/src/AogRing.h"
#include <cassert>
struct Row {uint32_t sequence, uptime;};
int main() {
  AogRing<Row,3> ring;
  ring.push({1,60});ring.push({2,120});ring.push({3,180});
  assert(ring.count==3 && ring.first()->sequence==1);
  // An ACK for a later row must not erase any unread history.
  ring.acknowledge(3);assert(ring.count==3);
  // No contact: keep a bounded recent window and account for every lost row.
  ring.push({4,240});assert(ring.count==3 && ring.dropped==1 && ring.first()->uptime==120);
  // Delayed ACK after overflow cannot erase a different row.
  ring.acknowledge(1);assert(ring.count==3);
  ring.acknowledge(2);assert(ring.count==2);
  ring.acknowledge(2);assert(ring.count==2); // duplicate delivery
  ring.acknowledge(3);ring.acknowledge(4);assert(!ring.first() && !ring.count);
  for(uint32_t n=5;n<300;++n) {ring.push({n,n*60});ring.acknowledge(n);}
  assert(ring.count==0 && ring.dropped==1); // wraparound does not corrupt the queue
}
