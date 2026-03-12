// FILE: BodyModule.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2002
// Desc:   BodyModule base class 
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "GameLogic/Module/BodyModule.h"

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void BodyModule::crc( Xfer *xfer )
{

	// call base class
	BehaviorModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer Method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void BodyModule::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// call base class
	BehaviorModule::xfer( xfer ); 

	// damage scalar
	xfer->xferReal( &m_damageScalar );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void BodyModule::loadPostProcess( void )
{

	// call base class
	BehaviorModule::loadPostProcess();
	
}  // end loadPostProcess
