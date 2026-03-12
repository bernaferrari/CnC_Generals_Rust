// GameCommon.h
// Part of header detangling
// John McDonald, Aug 2002

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/GameCommon.h"

const char *TheVeterancyNames[] = 
{
	"REGULAR",
	"VETERAN",
	"ELITE",
	"HEROIC",
	NULL
};

const char *TheRelationshipNames[] =
{
	"ENEMIES",
	"NEUTRAL",
	"ALLIES",
	NULL
};

//-------------------------------------------------------------------------------------------------
Real normalizeAngle(Real angle)
{
	DEBUG_ASSERTCRASH(!_isnan(angle), ("Angle is NAN in normalizeAngle!\n"));

	if( _isnan(angle) )
		return 0;// ARGH!!!! Don't assert and then not handle it!  Error bad!  Fix error!

	while (angle > PI) 
		angle -= 2*PI;

	while (angle <= -PI) 
		angle += 2*PI;

	return angle;
}

