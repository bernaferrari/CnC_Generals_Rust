#pragma once

#include "Lib/BaseType.h"

#ifdef _DEBUG
class CRC {
public:
    CRC() : crc(0) {}
    void computeCRC(const void *buf, Int len);
    void clear() { crc = 0; }
    UnsignedInt get();

private:
    void addCRC(UnsignedByte val);
    UnsignedInt crc;
};
#else
#error "The parity harness must compile the original debug CRC implementation"
#endif
