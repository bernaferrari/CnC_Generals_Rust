// FILE: ObjectStatusTypes.cpp ////////////////////////////////////////////////////////////////////
// Author: Kris, May 2003
// Desc:   Object status types that are stackable using the BitSet system. Used to be ObjectStatusBits
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"

#include "Common/ObjectStatusTypes.h"
#include "Common/BitFlagsIO.h"

const char* ObjectStatusMaskType::s_bitNameList[] = 
{
	"NONE",
	"DESTROYED",
	"CAN_ATTACK",					
	"UNDER_CONSTRUCTION",	
	"UNSELECTABLE",				
	"NO_COLLISIONS",				
	"NO_ATTACK",						
	"AIRBORNE_TARGET",			
	"PARACHUTING",	
	"REPULSOR",
	"HIJACKED",					
	"AFLAME",							
	"BURNED",							
	"WET",
	"IS_FIRING_WEAPON",
	"IS_BRAKING",
	"STEALTHED",
	"DETECTED",
	"CAN_STEALTH",
	"SOLD",
	"UNDERGOING_REPAIR",
	"RECONSTRUCTING",
	"MASKED",
	"IS_ATTACKING",
	"USING_ABILITY",
	"IS_AIMING_WEAPON",
	"NO_ATTACK_FROM_AI",
	"IGNORING_STEALTH",
	"IS_CARBOMB",
	"DECK_HEIGHT_OFFSET",
	"STATUS_RIDER1",
	"STATUS_RIDER2",
	"STATUS_RIDER3",
	"STATUS_RIDER4",
	"STATUS_RIDER5",
	"STATUS_RIDER6",
	"STATUS_RIDER7",
	"STATUS_RIDER8",
	"FAERIE_FIRE",
  "KILLING_SELF",
	"REASSIGN_PARKING",
	"BOOBY_TRAPPED",
	"IMMOBILE",
	"DISGUISED",
	"DEPLOYED",
	NULL
};

ObjectStatusMaskType OBJECT_STATUS_MASK_NONE;	// inits to all zeroes