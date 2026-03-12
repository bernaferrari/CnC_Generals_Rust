// FILE: ImmortalBody.cpp ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, April 2002
// Desc:	 Just like Active Body, but won't let health drop below 1
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Damage.h"
#include "GameLogic/Module/ImmortalBody.h"

// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ImmortalBody::ImmortalBody( Thing *thing, const ModuleData* moduleData ) 
						 : ActiveBody( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ImmortalBody::~ImmortalBody( void )
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void ImmortalBody::internalChangeHealth( Real delta )
{
	// Don't let anything changes us to below one hit point
	delta = max( delta, -getHealth() + 1 );

	// extend functionality, but I go first because I can't let you die and then fix it, I must prevent
	ActiveBody::internalChangeHealth( delta );

	// nothing -- never mark it as dead.
	DEBUG_ASSERTCRASH( (getHealth() > 0 && !getObject()->isEffectivelyDead() ), ("Immortal objects should never get marked as dead!"));

}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ImmortalBody::crc( Xfer *xfer )
{

	// extend base class
	ActiveBody::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void ImmortalBody::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	ActiveBody::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void ImmortalBody::loadPostProcess( void )
{

	// extend base class
	ActiveBody::loadPostProcess();

}  // end loadPostProcess
