// FILE: PreorderCreate.cpp ///////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, December 2002
// Desc:   When a building is created, set the preorder status if necessary
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Player.h"
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/PreorderCreate.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PreorderCreate::PreorderCreate( Thing *thing, const ModuleData* moduleData ) : CreateModule( thing, moduleData )
{

}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PreorderCreate::~PreorderCreate( void )
{

} 

//-------------------------------------------------------------------------------------------------
void PreorderCreate::onCreate( void )
{
}

//-------------------------------------------------------------------------------------------------
void PreorderCreate::onBuildComplete( void )
{
	if (getObject()->getControllingPlayer()->didPlayerPreorder())
	{
		getObject()->setModelConditionState( MODELCONDITION_PREORDER );
	}
	else
	{
		getObject()->clearModelConditionState( MODELCONDITION_PREORDER );
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void PreorderCreate::crc( Xfer *xfer )
{

	// extend base class
	CreateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void PreorderCreate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	CreateModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void PreorderCreate::loadPostProcess( void )
{

	// extend base class
	CreateModule::loadPostProcess();

}  // end loadPostProcess
