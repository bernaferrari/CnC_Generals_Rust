// DisabledTypes.cpp /////////////////////////////////////////////////////////////////////////////////////
// Kris Morness, September 2002

#include "PreRTS.h"

#include "Common/DisabledTypes.h"
#include "Common/BitFlagsIO.h"

const char* DisabledMaskType::s_bitNameList[] = 
{
	"DEFAULT",
	"DISABLED_HACKED",
	"DISABLED_EMP",
	"DISABLED_HELD",
	"DISABLED_PARALYZED",
	"DISABLED_UNMANNED",
	"DISABLED_UNDERPOWERED",
	"DISABLED_FREEFALL",
	
  "DISABLED_AWESTRUCK",
  "DISABLED_BRAINWASHED",
	"DISABLED_SUBDUED",

	"DISABLED_SCRIPT_DISABLED",
	"DISABLED_SCRIPT_UNDERPOWERED",

	NULL
};

DisabledMaskType DISABLEDMASK_NONE;	// inits to all zeroes
DisabledMaskType DISABLEDMASK_ALL;

void initDisabledMasks()
{
	SET_ALL_DISABLEDMASK_BITS( DISABLEDMASK_ALL );
}
