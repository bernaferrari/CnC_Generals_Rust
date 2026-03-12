// FILE: DestroyModule.cpp ////////////////////////////////////////////////////////////////////////
// Author: Colin Day, October 2002
// Desc:   Destroy module base class
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "GameLogic/Module/DestroyModule.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
DestroyModule::DestroyModule( Thing *thing, const ModuleData* moduleData ) 
							: BehaviorModule( thing, moduleData )
{

}  // end DestroyModule

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
DestroyModule::~DestroyModule( void )
{

}  // end ~DestroyModule

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void DestroyModule::crc( Xfer *xfer )
{

	// extend base class
	BehaviorModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void DestroyModule::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	BehaviorModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void DestroyModule::loadPostProcess( void )
{

	// extend base class
	BehaviorModule::loadPostProcess();

}  // end loadPostProcess

