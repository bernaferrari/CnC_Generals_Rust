// Only the platform/file dependencies are adapted. The oracle compiles the
// repository's original chunkio.cpp, chunkio.h and iostruct.h unchanged.
#pragma once
#define ALWAYS_H
#define BITTYPE_H
#define WWFILE_H
#include <algorithm>
#include <cassert>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>
using uint32 = std::uint32_t;
using uint8 = std::uint8_t;
using float32 = float;

class FileClass {
public:
    std::vector<uint8> bytes;
    int position = 0;
    int Seek(int offset, int origin = SEEK_CUR) {
        position = (origin == SEEK_SET ? 0 : origin == SEEK_END ? int(bytes.size()) : position) + offset;
        assert(position >= 0);
        return position;
    }
    int Tell() { return position; }
    int Write(const void *source, int length) {
        bytes.resize(std::max(bytes.size(), std::size_t(position + length)));
        std::memcpy(bytes.data() + position, source, length);
        position += length;
        return length;
    }
    int Read(void *destination, int length) {
        int available = std::max(0, int(bytes.size()) - position);
        int count = std::min(length, available);
        if (count) std::memcpy(destination, bytes.data() + position, count);
        position += count;
        return count;
    }
};
