// FILE: InternetHackContain.cpp //////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:   Contain module that just gives aiHackInternet command to passengers
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameLogic/Module/InternetHackContain.h"

#include "GameLogic/Object.h"
#include "GameLogic/Module/AIUpdate.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
InternetHackContainModuleData::InternetHackContainModuleData()
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void InternetHackContainModuleData::buildFieldParse(MultiIniFieldParse& p)
{
  TransportContainModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}



// PRIVATE ////////////////////////////////////////////////////////////////////////////////////////

// PUBLIC /////////////////////////////////////////////////////////////////////////////////////////
//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
InternetHackContain::InternetHackContain( Thing *thing, const ModuleData *moduleData ) : 
								 TransportContain( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
InternetHackContain::~InternetHackContain( void )
{

}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void InternetHackContain::onContaining( Object *rider, Bool wasSelected )
{
	TransportContain::onContaining( rider, wasSelected );

	if( rider->getAI() )
		rider->getAI()->aiHackInternet(CMD_FROM_AI);
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void InternetHackContain::crc( Xfer *xfer )
{

	// extend base class
	TransportContain::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void InternetHackContain::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	TransportContain::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void InternetHackContain::loadPostProcess( void )
{

	// extend base class
	TransportContain::loadPostProcess();

}  // end loadPostProcess
