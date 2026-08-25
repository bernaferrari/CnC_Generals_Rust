#pragma once

#include "Lib/BaseType.h"

extern void InitRandom(void);
extern void InitRandom(UnsignedInt seed);
extern void InitGameLogicRandom(UnsignedInt seed);
extern UnsignedInt GetGameLogicRandomSeed(void);
extern UnsignedInt GetGameLogicRandomSeedCRC(void);

extern Int GetGameClientRandomValue(int lo, int hi, char *file, int line);
extern Real GetGameClientRandomValueReal(Real lo, Real hi, char *file, int line);
extern Int GetGameLogicRandomValue(int lo, int hi, char *file, int line);
extern Real GetGameLogicRandomValueReal(Real lo, Real hi, char *file, int line);

class GameClientRandomVariable {
public:
    enum DistributionType { CONSTANT, UNIFORM, GAUSSIAN, TRIANGULAR, LOW_BIAS, HIGH_BIAS };
    static const char *DistributionTypeNames[];
    void setRange(Real low, Real high, DistributionType type = UNIFORM);
    Real getValue(void) const;
protected:
    DistributionType m_type;
    Real m_low, m_high;
};

class GameLogicRandomVariable {
public:
    enum DistributionType { CONSTANT, UNIFORM, GAUSSIAN, TRIANGULAR, LOW_BIAS, HIGH_BIAS };
    static const char *DistributionTypeNames[];
    void setRange(Real low, Real high, DistributionType type = UNIFORM);
    Real getValue(void) const;
protected:
    DistributionType m_type;
    Real m_low, m_high;
};

#define GameClientRandomValueReal(lo, hi) \
    GetGameClientRandomValueReal((lo), (hi), const_cast<char *>(__FILE__), __LINE__)
#define GameLogicRandomValueReal(lo, hi) \
    GetGameLogicRandomValueReal((lo), (hi), const_cast<char *>(__FILE__), __LINE__)
