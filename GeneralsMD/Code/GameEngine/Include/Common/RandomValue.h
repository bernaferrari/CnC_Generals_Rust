// RandomValue.h
// Random number generation system
// Author: Michael S. Booth, January 1998

#pragma once

#ifndef _RANDOM_VALUE_H_
#define _RANDOM_VALUE_H_

#include "Lib/BaseType.h"

extern void InitRandom( void );
extern void InitRandom( UnsignedInt seed );
extern void InitGameLogicRandom( UnsignedInt seed ); ///< Set the GameLogic seed to a known value at game start
extern UnsignedInt GetGameLogicRandomSeed( void );   ///< Get the seed (used for replays)
extern UnsignedInt GetGameLogicRandomSeedCRC( void );///< Get the seed (used for CRCs)

//--------------------------------------------------------------------------------------------------------------

#endif // _RANDOM_VALUE_H_
