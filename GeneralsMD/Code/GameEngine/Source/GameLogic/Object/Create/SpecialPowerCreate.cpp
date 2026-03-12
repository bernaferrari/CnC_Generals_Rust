// FILE: SpecialPowerCreate.h /////////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, May 2002
// Desc:   When a building is created, tell special powers to start counting down
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/SpecialPowerCreate.h"
#include "GameLogic/Module/SpecialPowerModule.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SpecialPowerCreate::SpecialPowerCreate( Thing *thing, const ModuleData* moduleData ) : CreateModule( thing, moduleData )
{

}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SpecialPowerCreate::~SpecialPowerCreate( void )
{

} 

//-------------------------------------------------------------------------------------------------
void SpecialPowerCreate::onCreate( void )
{
}

//-------------------------------------------------------------------------------------------------
void SpecialPowerCreate::onBuildComplete( void )
{
	if( ! shouldDoOnBuildComplete() )
		return;

	CreateModule::onBuildComplete(); // extend

	for (BehaviorModule** m = getObject()->getBehaviorModules(); *m; ++m)
	{
		SpecialPowerModuleInterface* sp = (*m)->getSpecialPower();
		if (!sp)
			continue;

		sp->onSpecialPowerCreation();
	}
} 

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void SpecialPowerCreate::crc( Xfer *xfer )
{

	// extend base class
	CreateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void SpecialPowerCreate::xfer( Xfer *xfer )
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
void SpecialPowerCreate::loadPostProcess( void )
{

	// extend base class
	CreateModule::loadPostProcess();

}  // end loadPostProcess
