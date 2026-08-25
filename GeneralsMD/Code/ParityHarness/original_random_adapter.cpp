// This translation unit deliberately textually includes the original units.
// The adapter exposes only the original GameLogic RNG state for the portable
// parity producer; no gameplay logic is reimplemented here.
#define _DEBUG 1

static_assert(sizeof(unsigned int) == 4, "GeneralsMD RNG requires 32-bit unsigned int");

#include "../GameEngine/Source/Common/crc.cpp"
#include "../GameEngine/Source/Common/RandomValue.cpp"

extern "C" const UnsignedInt *generalsmd_logic_seed(void)
{
    // The symbol has internal linkage in the original unit, but this wrapper
    // is in the same translation unit and can therefore inspect its exact
    // state without modifying the source implementation.
    return theGameLogicSeed;
}

extern "C" void generalsmd_init_logic_random(UnsignedInt seed)
{
    InitGameLogicRandom(seed);
}

extern "C" void generalsmd_draw_logic_random(void)
{
    (void)GetGameLogicRandomValue(0, 0x7fffffff, const_cast<char *>("parity"), 0);
}

extern "C" UnsignedInt generalsmd_logic_seed_crc(void)
{
    return GetGameLogicRandomSeedCRC();
}
