// FILE: SpecialAbility.cpp /////////////////////////////////////////////////////////////
// Author: Kris Morness, July 2002
// Desc:   This is the class that handles processing of any special attack from a unit. There are 
//         many different styles and rules for various attacks.
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Player.h"
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/SpecialAbility.h"

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
SpecialAbility::SpecialAbility( Thing *thing, const ModuleData *moduleData )
												: SpecialPowerModule( thing, moduleData )
{

}  // end SpecialAbility

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
SpecialAbility::~SpecialAbility( void )
{

}  // end ~SpecialAbility

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void SpecialAbility::doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions )
{
	if (getObject()->isDisabled())
		return;

	// sanity
	if( loc == NULL )
		return;

	// call the base class action cause we are *EXTENDING* functionality
	SpecialPowerModule::doSpecialPowerAtLocation( loc, angle, commandOptions );
}  

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void SpecialAbility::doSpecialPowerAtObject( Object *obj, UnsignedInt commandOptions )
{
	if (getObject()->isDisabled())
		return;

	if (!obj)
		return;

	// call the base class action cause we are *EXTENDING* functionality
	SpecialPowerModule::doSpecialPowerAtObject( obj, commandOptions );
}  


// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void SpecialAbility::doSpecialPower( UnsignedInt commandOptions )
{
	if (getObject()->isDisabled())
		return;

	// call the base class action cause we are *EXTENDING* functionality
	SpecialPowerModule::doSpecialPower( commandOptions );
}  

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void SpecialAbility::crc( Xfer *xfer )
{

	// extend base class
	SpecialPowerModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void SpecialAbility::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	SpecialPowerModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void SpecialAbility::loadPostProcess( void )
{

	// extend base class
	SpecialPowerModule::loadPostProcess();

}  // end loadPostProcess
