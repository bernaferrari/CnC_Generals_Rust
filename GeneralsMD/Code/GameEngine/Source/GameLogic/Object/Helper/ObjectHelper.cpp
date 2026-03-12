// FILE: ObjectHelper.cpp /////////////////////////////////////////////////////////////////////////
// Author: Colin Day, Steven Johnson, September 2002
// Desc:   Object helper module base class
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/ObjectHelper.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
ObjectHelper::~ObjectHelper( void )
{

}  // end ~ObjectHelper

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void ObjectHelper::sleepUntil(UnsignedInt when)
{
	if( getObject()->getStatusBits().test( OBJECT_STATUS_DESTROYED ) )
		return;

	// note the setWakeFrame(NEVER) actually awakens immediately, since NEVER==0.
	// when we get NEVER in this case, we really want to sleep forever.
	// so just special case it.
	UpdateSleepTime wakeDelay = (when == NEVER || when == FOREVER) ? 
																UPDATE_SLEEP_FOREVER :
																UPDATE_SLEEP(when - TheGameLogic->getFrame());
	setWakeFrame(getObject(), wakeDelay);
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ObjectHelper::crc( Xfer *xfer )
{

	// update module crc
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/* Xfer method
 * Version Info:
 * 1: Initial Version */
// ------------------------------------------------------------------------------------------------
void ObjectHelper::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// update module xfer
	UpdateModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void ObjectHelper::loadPostProcess( void )
{

	// update module post process
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
