#pragma once
#include <stdint.h>
template<typename T, uint8_t Capacity> class AogRing {
 public:
  T rows[Capacity]; uint8_t head = 0, count = 0; uint32_t dropped = 0;
  void push(const T& row) {
    if (count == Capacity) {head = (head + 1) % Capacity; --count; ++dropped;}
    rows[(head + count) % Capacity] = row; ++count;
  }
  const T* first() const {return count ? &rows[head] : nullptr;}
  void acknowledge(uint32_t sequence) {
    if (count && rows[head].sequence == sequence) {head = (head + 1) % Capacity; --count;}
  }
};
