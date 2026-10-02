#include "chunkio.h"
#include <fstream>

static float from_bits(uint32 bits) {
    float value;
    std::memcpy(&value, &bits, sizeof(value));
    return value;
}

int main(int argc, char **argv) {
    assert(argc == 3);
    const uint32 endian = 1;
    assert(*reinterpret_cast<const uint8 *>(&endian) == 1);
    IOVector2Struct v2 = {1.25f, -0.0f};
    IOVector3Struct v3 = {from_bits(0x7fc12345), -2.5f, 3.0f};
    IOVector4Struct v4 = {-1.0f, from_bits(0x7f800000), from_bits(0xff800000), from_bits(0x7fc54321)};
    IOQuaternionStruct quat = {{0.0f, 0.5f, -0.5f, 1.0f}};
    FileClass file;
    ChunkSaveClass save(&file);
    assert(save.Begin_Chunk(0x11223344));
    assert(save.Begin_Chunk(0xaabbccdd));
    assert(save.Write(v2) == 8);
    assert(save.Write(v3) == 12);
    assert(save.Write(v4) == 16);
    assert(save.Write(quat) == 16);
    assert(save.End_Chunk());
    assert(save.Begin_Chunk(0x10203040));
    assert(save.Begin_Micro_Chunk(0x7e));
    const uint8 payload[] = {0x10, 0x20, 0x30};
    assert(save.Write(payload, 3) == 3);
    assert(save.End_Micro_Chunk());
    assert(save.End_Chunk());
    assert(save.End_Chunk());
    std::ofstream(argv[1], std::ios::binary).write(reinterpret_cast<const char *>(file.bytes.data()), file.bytes.size());

    // Original raw-buffer Read writes a short prefix even when it returns zero; logical
    // chunk position advances only on a complete read. Record all short lengths.
    std::ofstream partial(argv[2], std::ios::binary);
    for (uint32 count = 0; count < sizeof(v4); ++count) {
        FileClass truncated;
        ChunkHeader header(9, sizeof(v4));
        truncated.Write(&header, sizeof(header));
        truncated.Write(&v4, count);
        truncated.Seek(0, SEEK_SET);
        ChunkLoadClass load(&truncated);
        assert(load.Open_Chunk());
        IOVector4Struct result = {1.25f, -2.5f, 3.0f, 4.0f};
        assert(load.Read(static_cast<void *>(&result), sizeof(result)) == 0);
        partial.write(reinterpret_cast<const char *>(&result), sizeof(result));
    }
}
