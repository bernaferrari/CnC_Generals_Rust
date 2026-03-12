// FILE: CreateModule.cpp /////////////////////////////////////////////////////////////////////////
// Author: Colin Day, October 2002
// Desc:   Create module base class
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "GameLogic/Module/CreateModule.h"

// ------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
CreateModule::CreateModule( Thing *thing, const ModuleData* moduleData ) 
						: BehaviorModule( thing, moduleData ),
						  m_needToRunOnBuildComplete(TRUE)
{

}  // end createModule

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
CreateModule::~CreateModule()
{

}  // end ~CreateModule

//-------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void CreateModule::crc( Xfer *xfer )
{

	// extend base class
	BehaviorModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void CreateModule::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	BehaviorModule::xfer( xfer );

	// need to run on build complete
	xfer->xferBool( &m_needToRunOnBuildComplete );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void CreateModule::loadPostProcess( void )
{

	// extend base class
	BehaviorModule::loadPostProcess();

}  // ene loadPostProcess


