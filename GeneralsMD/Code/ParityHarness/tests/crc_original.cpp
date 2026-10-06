// Fixture producer: link original_random_adapter.cpp, which includes the
// unchanged original crc.cpp and RandomValue.cpp. No CRC recurrence lives here.
#include "Common/CRC.h"
#include "Common/RandomValue.h"

#include <algorithm>
#include <array>
#include <cassert>
#include <climits>
#include <cstdio>
#include <cstring>
#include <vector>

extern "C" const UnsignedInt *generalsmd_logic_seed();
extern "C" void generalsmd_init_logic_random(UnsignedInt seed);
extern "C" UnsignedInt generalsmd_logic_seed_crc();

static void emit_bytes(const char *name, const std::vector<unsigned char> &bytes)
{
    CRC whole;
    whole.computeCRC(bytes.data(), static_cast<Int>(bytes.size()));
    const auto expected = whole.get();
    for (std::size_t split = 0; split <= bytes.size(); ++split) {
        CRC chunks;
        // data() may be null for an empty vector: do not offset that pointer.
        if (split) chunks.computeCRC(bytes.data(), static_cast<Int>(split));
        chunks.computeCRC(nullptr, 1);
        chunks.computeCRC(bytes.data(), 0);
        chunks.computeCRC(bytes.data(), -1);
        if (split < bytes.size()) {
            chunks.computeCRC(bytes.data() + split, static_cast<Int>(bytes.size() - split));
        }
        assert(chunks.get() == expected);
    }
    CRC single;
    for (const auto byte : bytes) single.computeCRC(&byte, 1);
    assert(single.get() == expected);
    whole.clear();
    assert(whole.get() == 0);
    whole.computeCRC(bytes.data(), static_cast<Int>(bytes.size()));
    assert(whole.get() == expected);
    std::printf("crc %s %08x\n", name, static_cast<unsigned int>(expected));
}

int main()
{
    static_assert(CHAR_BIT == 8, "original byte width");
    static_assert(sizeof(unsigned int) == 4, "original UnsignedInt width");
    static_assert(sizeof(UnsignedInt) == 4, "portability shim width");
    const unsigned int word = 0x01020304;
    const unsigned char little_endian[] = {4, 3, 2, 1};
    assert(std::memcmp(&word, little_endian, sizeof(word)) == 0);

    emit_bytes("empty", {});
    emit_bytes("max_byte", {255});
    emit_bytes("two_ones", {1, 1});
    std::vector<unsigned char> high(25, 0);
    high[0] = 128;
    emit_bytes("high_before", high);
    high.push_back(0);
    emit_bytes("high_after", high);
    std::vector<unsigned char> byte_overflow(31, 1);
    byte_overflow.push_back(255);
    emit_bytes("byte_overflow", byte_overflow);
    std::vector<unsigned char> carry_overflow(33, 1);
    emit_bytes("carry_overflow", carry_overflow);
    carry_overflow.back() = 255;
    emit_bytes("both_overflow", carry_overflow);
    std::vector<unsigned char> all_bytes;
    for (unsigned int i = 0; i < 768; ++i) all_bytes.push_back(i & 255);
    emit_bytes("all_bytes", all_bytes);
    const auto *word_bytes = reinterpret_cast<const unsigned char *>(&word);
    emit_bytes("word", {word_bytes, word_bytes + sizeof(word)});
    const unsigned int words[] = {0, 1, 0x01020304, 0x80000000, 0xffffffff, 0x89abcdef};
    const auto *mixed_bytes = reinterpret_cast<const unsigned char *>(words);
    emit_bytes("mixed_words", {mixed_bytes, mixed_bytes + sizeof(words)});

    for (const UnsignedInt seed : {0u, 1u, 0x80000000u, 0xffffffffu, 0x5eed00ccu}) {
        generalsmd_init_logic_random(seed);
        for (unsigned int draws = 0; draws <= 32; ++draws) {
            if (draws == 0 || draws == 1 || draws == 2 || draws == 7 || draws == 32) {
                std::array<UnsignedInt, 6> before;
                std::copy_n(generalsmd_logic_seed(), before.size(), before.begin());
                const auto crc = generalsmd_logic_seed_crc();
                assert(generalsmd_logic_seed_crc() == crc);
                assert(std::equal(before.begin(), before.end(), generalsmd_logic_seed()));
                std::printf("rng %08x %u %08x", static_cast<unsigned int>(seed), draws,
                            static_cast<unsigned int>(crc));
                for (const auto value : before) std::printf(" %08x", static_cast<unsigned int>(value));
                std::printf("\n");
            }
            if (draws < 32) {
                // A non-overflowing range consumes one original draw.
                (void)GetGameLogicRandomValue(0, 1000, const_cast<char *>("crc fixture"), 0);
            }
        }
    }
}
