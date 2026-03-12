// FILE: ObjectSMCHelper.cpp //////////////////////////////////////////////////////////////////////
// Author: Colin Day, Steven Johnson, September 2002
// Desc:   SMC helper
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/ObjectSMCHelper.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
ObjectSMCHelper::~ObjectSMCHelper( void )
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
UpdateSleepTime ObjectSMCHelper::update()
{
	// if we ever get here, stop this.
	getObject()->clearSpecialModelConditionStates();

	// then go back to sleep until we are forcibly awakened.
	return UPDATE_SLEEP_FOREVER; 
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ObjectSMCHelper::crc( Xfer *xfer )
{

	// object helper crc
	ObjectHelper::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info;
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void ObjectSMCHelper::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// object helper base class
	ObjectHelper::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void ObjectSMCHelper::loadPostProcess( void )
{

	// object helper base class
	ObjectHelper::loadPostProcess();

}  // end loadPostProcess

