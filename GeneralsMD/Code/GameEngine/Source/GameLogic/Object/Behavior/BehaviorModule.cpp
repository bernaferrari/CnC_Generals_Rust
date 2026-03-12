// FILE: BehaviorModule.cpp ///////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2002
// Desc:   Implementaion for anything in the base BehaviorModule
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "GameLogic/Module/BehaviorModule.h"

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void BehaviorModule::crc( Xfer *xfer )
{

	// call base class
	ObjectModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer Method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void BehaviorModule::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// call base class
	ObjectModule::xfer( xfer );

}  // xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void BehaviorModule::loadPostProcess( void )
{

	// call base class
	ObjectModule::loadPostProcess();

}  // end loadPostProcess