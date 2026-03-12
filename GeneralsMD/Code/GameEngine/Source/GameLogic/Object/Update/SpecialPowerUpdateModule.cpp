// FILE: SpecialPowerUpdateModule.cpp ///////////////////////////////////////////////////////////////////
// Author: Kris Morness, July 2003
// Desc:	 Special power update module interface
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Player.h"
#include "Common/PlayerList.h"
#include "Common/Science.h"
#include "Common/SpecialPower.h"
#include "Common/Xfer.h"

#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/SpecialPowerUpdateModule.h"
#include "GameLogic/Module/SpecialPowerModule.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

//-------------------------------------------------------------------------------------------------
SpecialPowerUpdateModule::SpecialPowerUpdateModule( Thing *thing, const ModuleData* moduleData ) : UpdateModule( thing, moduleData ) 
{
}

//-------------------------------------------------------------------------------------------------
SpecialPowerUpdateModule::~SpecialPowerUpdateModule()
{
}

//-------------------------------------------------------------------------------------------------
Bool SpecialPowerUpdateModule::doesSpecialPowerUpdatePassScienceTest() const
{
	//Kris: July 24, 2003 -- Added an additional optional check for objects with multiple SpecialPowerModules referencing
	//the same SpecialPowerTemplate but with special ScienceType checks. An example of this is the three 
	//SpectreGunshipDeploymentUpdate modules inside AirF_AmericaCommandCenter. Each one has a different duration which
	//is hooked into different objects. This sucked and became necessary because the way the stackable changable icon system 
	//for multilevel buttons.
	ScienceType science = getExtraRequiredScience();
	if( science != SCIENCE_INVALID )
	{
		if( getObject()->getControllingPlayer()->hasScience( science ) )
		{
			//We have the science.
			return TRUE;
		}
		//We don't have the science.
		return FALSE;
	}
	//We don't have this extra check
	return TRUE;
}


//------------------------------------------------------------------------------------------------
/** CRC */
//------------------------------------------------------------------------------------------------
void SpecialPowerUpdateModule::crc( Xfer *xfer )
{

	// extend base class 
	UpdateModule::crc( xfer );

}  // end crc

//------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
//------------------------------------------------------------------------------------------------
void SpecialPowerUpdateModule::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );
}  // end xfer

//------------------------------------------------------------------------------------------------
/** Load post process */
//------------------------------------------------------------------------------------------------
void SpecialPowerUpdateModule::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
