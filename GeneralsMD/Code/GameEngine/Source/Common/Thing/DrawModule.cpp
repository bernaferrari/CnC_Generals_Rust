// FILE: DrawModule.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2002
// Desc:   Draw module base class
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/DrawModule.h"
#include "Common/Xfer.h"

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void DrawModule::crc( Xfer *xfer )
{

	// extend base class
	DrawableModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method	
	* Version Info;
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void DrawModule::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	DrawableModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void DrawModule::loadPostProcess( void )
{

	// extend base class
	DrawableModule::loadPostProcess();

}  // end loadPostProcess

