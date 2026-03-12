// FILE: KeepObjectDie.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, November 2002
// Desc:   Die module for things that want to leave rubble in the world and don't have other die
//         modules. This fixes civilian buildings that don't have garrison contains. Garrison
//         contains have a die module built in, so these buildings need something. Without it
//         they default to the destroydie module which outright removes the object.
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Module/KeepObjectDie.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
KeepObjectDie::KeepObjectDie( Thing *thing, const ModuleData* moduleData ) : DieModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
KeepObjectDie::~KeepObjectDie( void )
{
}

//-------------------------------------------------------------------------------------------------
/** The die callback. */
//-------------------------------------------------------------------------------------------------
void KeepObjectDie::onDie( const DamageInfo *damageInfo )
{
	if( !isDieApplicable(damageInfo) )
	{
		return;
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void KeepObjectDie::crc( Xfer *xfer )
{

	// extend base class
	DieModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void KeepObjectDie::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	DieModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void KeepObjectDie::loadPostProcess( void )
{

	// extend base class
	DieModule::loadPostProcess();

}  // end loadPostProcess
